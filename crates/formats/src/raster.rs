use std::fmt;
use std::io::Cursor;

use image::{ExtendedColorType, ImageDecoder, ImageFormat, ImageReader};

/// Raster containers supported by the first release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterFormat {
    Png,
    Jpeg,
    WebP,
    Tiff,
    Bmp,
    Gif,
}

impl RasterFormat {
    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::WebP => ImageFormat::WebP,
            Self::Tiff => ImageFormat::Tiff,
            Self::Bmp => ImageFormat::Bmp,
            Self::Gif => ImageFormat::Gif,
        }
    }

    fn from_image_format(format: ImageFormat) -> Result<Self, RasterError> {
        match format {
            ImageFormat::Png => Ok(Self::Png),
            ImageFormat::Jpeg => Ok(Self::Jpeg),
            ImageFormat::WebP => Ok(Self::WebP),
            ImageFormat::Tiff => Ok(Self::Tiff),
            ImageFormat::Bmp => Ok(Self::Bmp),
            ImageFormat::Gif => Ok(Self::Gif),
            _ => Err(RasterError::UnsupportedFormat),
        }
    }
}

/// An owned, row-major RGBA8 buffer with straight (not premultiplied) alpha.
/// Dimensions are nonzero and the buffer is exactly `width * height * 4` bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RasterImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, RasterError> {
        let expected = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(pixels.len() as u64) {
            return Err(RasterError::InvalidPixels);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

/// Detected source container and normalized pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedRaster {
    pub format: RasterFormat,
    pub image: RasterImage,
}

/// Invalid pixel data, unsupported containers, or an underlying codec failure.
#[derive(Debug)]
pub enum RasterError {
    InvalidPixels,
    UnsupportedFormat,
    Codec(image::ImageError),
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPixels => {
                f.write_str("expected nonzero dimensions and exactly width * height * 4 RGBA bytes")
            }
            Self::UnsupportedFormat => f.write_str(
                "unsupported raster format (expected PNG, JPEG, WebP, TIFF, BMP or GIF)",
            ),
            Self::Codec(error) => write!(f, "raster codec: {error}"),
        }
    }
}

impl std::error::Error for RasterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            _ => None,
        }
    }
}

impl From<image::ImageError> for RasterError {
    fn from(error: image::ImageError) -> Self {
        Self::Codec(error)
    }
}

/// Decode by file signature, not extension, using the codec's default resource
/// limits. EXIF orientation is applied before returning RGBA8 pixels. GIF input
/// yields the first frame only; multi-page TIFF yields its first image.
pub fn decode(bytes: &[u8]) -> Result<DecodedRaster, RasterError> {
    let detected = image::guess_format(bytes).map_err(|_| RasterError::UnsupportedFormat)?;
    let format = RasterFormat::from_image_format(detected)?;
    let reader = ImageReader::with_format(Cursor::new(bytes), detected);
    let mut decoder = reader.into_decoder()?;
    // into_decoder configures codec limits but does not perform ImageReader's
    // output-buffer reservation. Check both decoded and normalized RGBA sizes
    // before allocating, including for codecs with best-effort internal limits.
    let (width, height) = decoder.dimensions();
    let rgba_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(RasterError::InvalidPixels)?;
    image::Limits::default().reserve(decoder.total_bytes().max(rgba_bytes))?;
    let orientation = decoder.orientation()?;
    let mut image = image::DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    let rgba = image.into_rgba8();
    let image = RasterImage::new(rgba.width(), rgba.height(), rgba.into_raw())?;
    Ok(DecodedRaster { format, image })
}

/// Encode a flat image with no copied metadata or editable payload.
/// PNG, WebP (lossless), TIFF and BMP retain RGBA8. JPEG uses quality 90 and
/// composites transparency against white. GIF quantizes to at most 256 colors
/// with binary transparency; it writes one frame.
pub fn encode(image: &RasterImage, format: RasterFormat) -> Result<Vec<u8>, RasterError> {
    let mut output = Cursor::new(Vec::new());
    if format == RasterFormat::Jpeg {
        let rgb: Vec<u8> = image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| {
                let alpha = u32::from(pixel[3]);
                pixel[..3].iter().map(move |&channel| {
                    ((u32::from(channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
                })
            })
            .collect();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 90).encode(
            &rgb,
            image.width,
            image.height,
            ExtendedColorType::Rgb8,
        )?;
    } else {
        image::write_buffer_with_format(
            &mut output,
            &image.pixels,
            image.width,
            image.height,
            ExtendedColorType::Rgba8,
            format.image_format(),
        )?;
    }
    Ok(output.into_inner())
}
