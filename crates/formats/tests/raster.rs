use std::io::Cursor;

use image::{ImageEncoder, Rgba, RgbaImage};
use scribblecap_formats::{RasterFormat, RasterImage, decode, encode};

const FORMATS: [RasterFormat; 6] = [
    RasterFormat::Png,
    RasterFormat::Jpeg,
    RasterFormat::WebP,
    RasterFormat::Tiff,
    RasterFormat::Bmp,
    RasterFormat::Gif,
];

fn sample() -> RasterImage {
    RasterImage::new(
        3,
        2,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 0, 255, 255, 255,
            255, 0, 255, 255,
        ],
    )
    .unwrap()
}

#[test]
fn every_format_saves_and_reopens_by_signature() {
    let input = sample();
    for format in FORMATS {
        let bytes = encode(&input, format).unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.format, format);
        assert_eq!((decoded.image.width(), decoded.image.height()), (3, 2));
        // Also verify the flat file through the underlying ordinary image reader.
        let ordinary = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(ordinary.as_raw(), decoded.image.pixels());
        if format != RasterFormat::Jpeg {
            assert_eq!(decoded.image, input, "{format:?}");
        }
    }
}

#[test]
fn lossless_formats_preserve_straight_alpha() {
    let input = RasterImage::new(2, 1, vec![12, 34, 56, 128, 200, 100, 50, 255]).unwrap();
    for format in [
        RasterFormat::Png,
        RasterFormat::WebP,
        RasterFormat::Tiff,
        RasterFormat::Bmp,
    ] {
        assert_eq!(
            decode(&encode(&input, format).unwrap()).unwrap().image,
            input,
            "{format:?}"
        );
    }
}

#[test]
fn jpeg_composites_transparency_on_white() {
    let input = RasterImage::new(8, 8, [255, 0, 0, 0].repeat(64)).unwrap();
    let output = decode(&encode(&input, RasterFormat::Jpeg).unwrap())
        .unwrap()
        .image;
    assert!(
        output
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] >= 250 && p[1] >= 250 && p[2] >= 250 && p[3] == 255)
    );
}

#[test]
fn jpeg_preserves_solid_color_with_expected_loss_and_alpha_blending() {
    let input = RasterImage::new(16, 16, [255, 0, 0, 128].repeat(256)).unwrap();
    let output = decode(&encode(&input, RasterFormat::Jpeg).unwrap())
        .unwrap()
        .image;
    for pixel in output.pixels().as_chunks::<4>().0.iter() {
        for (actual, expected) in pixel.iter().zip([255u8, 127, 127, 255]) {
            assert!(actual.abs_diff(expected) <= 3, "{pixel:?}");
        }
    }
}

#[test]
fn gif_uses_binary_transparency() {
    let input = RasterImage::new(2, 1, vec![255, 0, 0, 0, 0, 0, 255, 128]).unwrap();
    let output = decode(&encode(&input, RasterFormat::Gif).unwrap())
        .unwrap()
        .image;
    assert_eq!(output.pixels()[3], 0);
    assert_eq!(&output.pixels()[4..], &[0, 0, 255, 255]);
}

#[test]
fn animated_gif_opens_only_first_frame() {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for color in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            encoder
                .encode_frame(image::Frame::new(RgbaImage::from_pixel(2, 1, Rgba(color))))
                .unwrap();
        }
    }
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.format, RasterFormat::Gif);
    assert_eq!(decoded.image.pixels(), [255, 0, 0, 255].repeat(2));
}

// Synthetic TIFF/Exif IFD with one SHORT Orientation tag, no proprietary input.
fn exif(orientation: u16) -> Vec<u8> {
    let mut bytes = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    bytes.extend_from_slice(&orientation.to_le_bytes());
    bytes.extend_from_slice(&[0; 6]);
    bytes
}

#[test]
fn png_applies_all_eight_exif_orientations() {
    let input = sample();
    // Row-major indices in the original 3x2 image, independently enumerated.
    let orders = [
        vec![0, 1, 2, 3, 4, 5],
        vec![2, 1, 0, 5, 4, 3],
        vec![5, 4, 3, 2, 1, 0],
        vec![3, 4, 5, 0, 1, 2],
        vec![0, 3, 1, 4, 2, 5],
        vec![3, 0, 4, 1, 5, 2],
        vec![5, 2, 4, 1, 3, 0],
        vec![2, 5, 1, 4, 0, 3],
    ];
    for (index, order) in orders.iter().enumerate() {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_exif_metadata(exif((index + 1) as u16)).unwrap();
        encoder
            .write_image(input.pixels(), 3, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        let output = decode(&bytes).unwrap().image;
        let dimensions = if index < 4 { (3, 2) } else { (2, 3) };
        assert_eq!((output.width(), output.height()), dimensions);
        let expected: Vec<u8> = order
            .iter()
            .flat_map(|&i| input.pixels()[i * 4..i * 4 + 4].iter().copied())
            .collect();
        assert_eq!(output.pixels(), expected, "orientation {}", index + 1);
        // Saving transformed pixels must not carry the old orientation forward.
        assert_eq!(
            decode(&encode(&output, RasterFormat::Png).unwrap())
                .unwrap()
                .image,
            output
        );
    }
}

#[test]
fn jpeg_orientation_is_applied() {
    let input = sample();
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95);
    encoder.set_exif_metadata(exif(6)).unwrap();
    encoder
        .encode(
            &input
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| p[..3].iter().copied())
                .collect::<Vec<_>>(),
            3,
            2,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    let unrotated = image::load_from_memory(&bytes).unwrap().to_rgba8();
    let decoded = decode(&bytes).unwrap();
    assert_eq!((decoded.image.width(), decoded.image.height()), (2, 3));
    assert_eq!(
        decoded.image.pixels(),
        image::imageops::rotate90(&unrotated).as_raw()
    );
}

#[test]
fn webp_orientation_is_applied() {
    let input = sample();
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
    encoder.set_exif_metadata(exif(8)).unwrap();
    encoder
        .write_image(input.pixels(), 3, 2, image::ExtendedColorType::Rgba8)
        .unwrap();
    let output = decode(&bytes).unwrap().image;
    assert_eq!((output.width(), output.height()), (2, 3));
    let expected: Vec<u8> = [2, 5, 1, 4, 0, 3]
        .iter()
        .flat_map(|&i| input.pixels()[i * 4..i * 4 + 4].iter().copied())
        .collect();
    assert_eq!(output.pixels(), expected);
}

#[test]
fn tiff_orientation_is_applied() {
    let input = sample();
    // Minimal uncompressed RGB TIFF. Offsets point past the 10-entry IFD and
    // the three BitsPerSample values. Tag 274 specifies a horizontal mirror.
    let entries: [(u16, u16, u32, u32); 10] = [
        (256, 4, 1, 3),
        (257, 4, 1, 2),
        (258, 3, 3, 134),
        (259, 3, 1, 1),
        (262, 3, 1, 2),
        (273, 4, 1, 140),
        (274, 3, 1, 2),
        (277, 3, 1, 3),
        (278, 4, 1, 2),
        (279, 4, 1, 18),
    ];
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    bytes.extend_from_slice(&10u16.to_le_bytes());
    for (tag, kind, count, value) in entries {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[8, 0, 8, 0, 8, 0]);
    bytes.extend(
        input
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| p[..3].iter().copied()),
    );
    let output = decode(&bytes).unwrap().image;
    let expected: Vec<u8> = [2, 1, 0, 5, 4, 3]
        .iter()
        .flat_map(|&i| input.pixels()[i * 4..i * 4 + 4].iter().copied())
        .collect();
    assert_eq!(output.pixels(), expected);
}

#[test]
fn rejects_oversized_output_before_allocating_pixels() {
    // Valid BMP headers declaring a 16,384 x 16,384 24-bit bitmap, with no
    // pixel payload. This must hit the output budget, not attempt a huge buffer.
    let mut bytes = vec![0u8; 54];
    bytes[..2].copy_from_slice(b"BM");
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&16_384i32.to_le_bytes());
    bytes[22..26].copy_from_slice(&16_384i32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    let result = decode(&bytes);
    assert!(
        matches!(
            result,
            Err(scribblecap_formats::RasterError::Codec(
                image::ImageError::Limits(_)
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn rejects_unknown_truncated_and_out_of_scope_images() {
    assert!(decode(b"not an image").is_err());
    for format in FORMATS {
        let bytes = encode(&sample(), format).unwrap();
        assert!(decode(&bytes[..bytes.len().min(16)]).is_err(), "{format:?}");
    }
    assert!(decode(b"P6\n1 1\n255\n\xff\0\0").is_err());
}

#[test]
fn rejects_invalid_pixel_buffers() {
    for (w, h, bytes) in [
        (0, 1, vec![]),
        (1, 0, vec![]),
        (2, 2, vec![0; 15]),
        (1, 1, vec![0; 5]),
    ] {
        assert!(RasterImage::new(w, h, bytes).is_err());
    }
    assert!(RasterImage::new(u32::MAX, u32::MAX, vec![]).is_err());
}

#[test]
fn reads_independently_encoded_grayscale_and_sixteen_bit_pngs() {
    let input =
        image::ImageBuffer::<image::Luma<u16>, _>::from_raw(2, 1, vec![0u16, u16::MAX]).unwrap();
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma16(input)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    assert_eq!(
        decode(bytes.get_ref()).unwrap().image.pixels(),
        &[0, 0, 0, 255, 255, 255, 255, 255]
    );
}
