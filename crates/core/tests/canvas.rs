use scribblecap_core::canvas::{CanvasOperation::*, transform};
fn pixels(values: &[u8]) -> Vec<u8> {
    values.iter().flat_map(|&v| [v, 0, 0, v]).collect()
}
fn values(pixels: &[u8]) -> Vec<u8> {
    pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect()
}
#[test]
fn rotations_and_reflections_keep_all_channels_and_are_reversible() {
    let source = pixels(&[1, 2, 3, 4, 5, 6]);
    let cw = transform(3, 2, &source, RotateClockwise).unwrap();
    assert_eq!((cw.width, cw.height), (2, 3));
    assert_eq!(values(&cw.pixels), [4, 1, 5, 2, 6, 3]);
    assert_eq!(
        transform(2, 3, &cw.pixels, RotateCounterclockwise)
            .unwrap()
            .pixels,
        source
    );
    for (operation, expected) in [
        (FlipHorizontal, vec![3, 2, 1, 6, 5, 4]),
        (FlipVertical, vec![4, 5, 6, 1, 2, 3]),
    ] {
        let image = transform(3, 2, &source, operation).unwrap();
        assert_eq!(values(&image.pixels), expected);
        assert_eq!(
            transform(3, 2, &image.pixels, operation).unwrap().pixels,
            source
        );
    }
}
#[test]
fn crop_expand_and_resize_have_exact_pixel_placement() {
    let source = pixels(&[1, 2, 3, 4, 5, 6]);
    let cropped = transform(
        3,
        2,
        &source,
        Crop {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        },
    )
    .unwrap();
    assert_eq!(values(&cropped.pixels), [2, 3, 5, 6]);
    let expanded = transform(
        3,
        2,
        &source,
        Expand {
            width: 4,
            height: 4,
            x: 1,
            y: 1,
        },
    )
    .unwrap();
    assert_eq!(
        values(&expanded.pixels),
        [0, 0, 0, 0, 0, 1, 2, 3, 0, 4, 5, 6, 0, 0, 0, 0]
    );
    assert_eq!(
        transform(
            4,
            4,
            &expanded.pixels,
            Crop {
                x: 1,
                y: 1,
                width: 3,
                height: 2
            }
        )
        .unwrap()
        .pixels,
        source
    );
    assert_eq!(
        values(
            &transform(
                3,
                2,
                &source,
                Resize {
                    width: 6,
                    height: 1
                }
            )
            .unwrap()
            .pixels
        ),
        [1, 1, 2, 2, 3, 3]
    );
}
#[test]
fn rejects_invalid_inputs_and_overflow_before_allocating() {
    let source = pixels(&[1]);
    for op in [
        Resize {
            width: 0,
            height: 1,
        },
        Resize {
            width: u32::MAX,
            height: u32::MAX,
        },
        Crop {
            x: u32::MAX,
            y: 0,
            width: 2,
            height: 1,
        },
        Expand {
            width: 1,
            height: 1,
            x: 1,
            y: 0,
        },
    ] {
        assert!(transform(1, 1, &source, op).is_err());
    }
    assert!(transform(2, 1, &source, FlipVertical).is_err());
    assert!(transform(u32::MAX, u32::MAX, &[], FlipVertical).is_err());
}
