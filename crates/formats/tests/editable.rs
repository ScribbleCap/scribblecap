use scribblecap_core::{
    Arrow, Background, Color, Document, Element, ElementId, Point, Resource, ResourceId, Shape,
    Size,
};
use scribblecap_formats::{
    EditableDocument, EditableError, RasterFormat, RasterImage, ResourceData, decode, encode,
    encode_editable, extract_editable, open,
};

const FORMATS: [RasterFormat; 3] = [RasterFormat::Png, RasterFormat::Jpeg, RasterFormat::WebP];

fn original() -> RasterImage {
    RasterImage::new(24, 16, [240, 240, 240, 255].repeat(24 * 16)).unwrap()
}

fn bundle() -> EditableDocument {
    let mut document = Document::new(Size::new(24.0, 16.0));
    let id = ResourceId::from("original-background");
    document.resources.push(Resource {
        id: id.clone(),
        media_type: "image/png".into(),
    });
    document.background = Some(Background {
        resource: id.clone(),
        origin: Point::new(0.0, 0.0),
    });
    document.elements.push(Element {
        id: ElementId::from("arrow"),
        shape: Shape::Arrow(Arrow {
            start: Point::new(3.0, 8.0),
            end: Point::new(20.0, 8.0),
            width: 4.0,
            color: Color::rgb(255, 0, 0),
            head_at_start: false,
        }),
    });
    EditableDocument::new(
        document,
        vec![ResourceData {
            id,
            bytes: encode(&original(), RasterFormat::Png).unwrap(),
        }],
    )
    .unwrap()
}

fn preview(editable: &EditableDocument) -> RasterImage {
    let original = decode(&editable.resources[0].bytes).unwrap().image;
    let background = scribblecap_core::render::BackgroundImage::new(
        &editable.resources[0].id,
        original.width(),
        original.height(),
        original.pixels(),
    )
    .unwrap();
    let image = scribblecap_core::render::render(&editable.document, Some(background)).unwrap();
    RasterImage::new(image.width(), image.height(), image.into_pixels()).unwrap()
}

#[test]
fn all_containers_round_trip_document_resources_and_normal_preview() {
    let editable = bundle();
    let pixels = preview(&editable);
    for format in FORMATS {
        let bytes = encode_editable(&pixels, format, &editable).unwrap();
        assert_eq!(extract_editable(&bytes).unwrap(), Some(editable.clone()));
        let opened = open(&bytes).unwrap();
        assert_eq!(opened.editable, Some(editable.clone()));
        assert_eq!(opened.raster.format, format);
        // An ordinary decoder, without knowledge of ScribbleCap, still sees the preview.
        assert_eq!(
            opened.raster.image.pixels(),
            image::load_from_memory(&bytes).unwrap().to_rgba8().as_raw()
        );
        if format != RasterFormat::Jpeg {
            assert_eq!(opened.raster.image, pixels);
        }
    }
}

#[test]
fn reopening_and_removing_annotations_recovers_the_original_not_the_preview() {
    let editable = bundle();
    let flattened = preview(&editable);
    assert_ne!(flattened, original());
    for format in FORMATS {
        let bytes = encode_editable(&flattened, format, &editable).unwrap();
        let mut restored = extract_editable(&bytes).unwrap().unwrap();
        restored.document.elements.clear();
        assert_eq!(preview(&restored), original());
    }
}

#[test]
fn plain_files_have_no_editable_payload() {
    for format in [
        RasterFormat::Png,
        RasterFormat::Jpeg,
        RasterFormat::WebP,
        RasterFormat::Tiff,
        RasterFormat::Bmp,
        RasterFormat::Gif,
    ] {
        let bytes = encode(&original(), format).unwrap();
        assert!(extract_editable(&bytes).unwrap().is_none());
        assert!(open(&bytes).unwrap().editable.is_none());
    }
}

#[test]
fn repeated_saves_are_deterministic_and_replace_old_resource_data() {
    let mut editable = bundle();
    let id = ResourceId::from("extra");
    editable.document.resources.push(Resource {
        id: id.clone(),
        media_type: "application/octet-stream".into(),
    });
    editable.resources.push(ResourceData {
        id,
        bytes: b"OLD-PRIVATE-RESOURCE-DO-NOT-RETAIN-123456789".to_vec(),
    });
    for format in FORMATS {
        let first = encode_editable(&original(), format, &editable).unwrap();
        assert_eq!(
            first,
            encode_editable(&original(), format, &editable).unwrap()
        );
        let mut reopened = extract_editable(&first).unwrap().unwrap();
        reopened.document.resources.pop();
        reopened.resources.pop();
        let second = encode_editable(&original(), format, &reopened).unwrap();
        assert!(
            !second
                .windows(40)
                .any(|w| w.starts_with(b"OLD-PRIVATE-RESOURCE-DO-NOT-RETAIN"))
        );
        assert_eq!(extract_editable(&second).unwrap(), Some(bundle()));
    }
}

#[test]
fn resources_must_match_the_document_inventory_exactly() {
    let editable = bundle();
    assert!(EditableDocument::new(editable.document.clone(), vec![]).is_err());
    let mut resources = editable.resources.clone();
    resources.push(resources[0].clone());
    assert!(EditableDocument::new(editable.document.clone(), resources).is_err());
    let mut wrong = editable.resources.clone();
    wrong[0].id = ResourceId::from("wrong");
    assert!(EditableDocument::new(editable.document.clone(), wrong).is_err());
    // Public data may be edited after construction, so save validates again.
    let mut invalid = editable;
    invalid.resources.clear();
    assert!(encode_editable(&original(), RasterFormat::Png, &invalid).is_err());
}

#[test]
fn mismatched_preview_dimensions_and_flat_only_formats_are_rejected() {
    let small = RasterImage::new(1, 1, vec![0; 4]).unwrap();
    assert!(encode_editable(&small, RasterFormat::Png, &bundle()).is_err());
    for format in [RasterFormat::Tiff, RasterFormat::Bmp, RasterFormat::Gif] {
        assert!(matches!(
            encode_editable(&original(), format, &bundle()),
            Err(EditableError::UnsupportedFormat)
        ));
    }
}

#[test]
fn transparent_webp_has_valid_extended_header_and_riff_size() {
    let mut editable = bundle();
    editable.document.elements.clear();
    let pixels = RasterImage::new(24, 16, [90, 120, 200, 128].repeat(24 * 16)).unwrap();
    let bytes = encode_editable(&pixels, RasterFormat::WebP, &editable).unwrap();
    assert_eq!(&bytes[12..16], b"VP8X");
    assert_eq!(bytes[20] & 0x10, 0x10);
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
        bytes.len()
    );
    assert_eq!(decode(&bytes).unwrap().image, pixels);
}

#[test]
fn damaged_payload_is_an_error_even_when_the_image_still_opens() {
    for format in FORMATS {
        let mut bytes = encode_editable(&original(), format, &bundle()).unwrap();
        let start = bytes.windows(8).position(|w| w == b"SCAPDOC\0").unwrap();
        bytes[start + 30] ^= 1;
        assert!(extract_editable(&bytes).is_err(), "{format:?}");
        assert!(open(&bytes).is_err(), "{format:?}");
        if format != RasterFormat::Png {
            assert!(image::load_from_memory(&bytes).is_ok());
        }
    }
}

#[test]
fn truncation_and_forged_lengths_are_rejected_without_panicking() {
    for format in FORMATS {
        let bytes = encode_editable(&original(), format, &bundle()).unwrap();
        for end in [0, 1, 8, 16, bytes.len() / 2, bytes.len() - 1] {
            assert!(open(&bytes[..end]).is_err(), "{format:?} prefix {end}");
        }
        let mut forged = bytes;
        match format {
            RasterFormat::Png => forged[8..12].copy_from_slice(&u32::MAX.to_be_bytes()),
            RasterFormat::Jpeg => {
                let at = forged.len() - 16;
                forged[at..at + 8].copy_from_slice(&u64::MAX.to_be_bytes());
            }
            RasterFormat::WebP => forged[4..8].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => unreachable!(),
        }
        assert!(extract_editable(&forged).is_err());
    }
}

#[test]
fn stale_editable_data_is_rejected_if_the_flattened_image_changes() {
    let changed = RasterImage::new(24, 16, [0, 30, 255, 255].repeat(24 * 16)).unwrap();
    for format in FORMATS {
        let first = encode_editable(&original(), format, &bundle()).unwrap();
        let mut second = encode_editable(&changed, format, &bundle()).unwrap();
        let source = first.windows(8).position(|w| w == b"SCAPDOC\0").unwrap();
        let dest = second.windows(8).position(|w| w == b"SCAPDOC\0").unwrap();
        let length = match format {
            RasterFormat::Png => {
                u32::from_be_bytes(first[source - 8..source - 4].try_into().unwrap()) as usize
            }
            RasterFormat::Jpeg => first.len() - source - 16,
            RasterFormat::WebP => {
                u32::from_le_bytes(first[source - 4..source].try_into().unwrap()) as usize
            }
            _ => unreachable!(),
        };
        second[dest..dest + length].copy_from_slice(&first[source..source + length]);
        if format == RasterFormat::Png {
            let crc = crc32fast::hash(&second[dest - 4..dest + length]);
            second[dest + length..dest + length + 4].copy_from_slice(&crc.to_be_bytes());
        }
        assert!(
            matches!(
                extract_editable(&second),
                Err(EditableError::PreviewChanged)
            ),
            "{format:?}"
        );
        assert!(image::load_from_memory(&second).is_ok());
    }
}

#[test]
fn duplicate_metadata_cannot_hide_an_older_document() {
    for format in FORMATS {
        let mut bytes = encode_editable(&original(), format, &bundle()).unwrap();
        let start = bytes.windows(8).position(|w| w == b"SCAPDOC\0").unwrap();
        match format {
            RasterFormat::Png => {
                let length =
                    u32::from_be_bytes(bytes[start - 8..start - 4].try_into().unwrap()) as usize;
                let chunk = bytes[start - 8..start + length + 4].to_vec();
                let at = bytes.len() - 12;
                bytes.splice(at..at, chunk);
            }
            RasterFormat::WebP => {
                let length =
                    u32::from_le_bytes(bytes[start - 4..start].try_into().unwrap()) as usize;
                let chunk = bytes[start - 8..start + length + length % 2].to_vec();
                bytes.extend_from_slice(&chunk);
                let size = (bytes.len() - 8) as u32;
                bytes[4..8].copy_from_slice(&size.to_le_bytes());
            }
            RasterFormat::Jpeg => {
                let extra = bytes[start..].to_vec();
                bytes.extend_from_slice(&extra);
            }
            _ => unreachable!(),
        }
        assert!(extract_editable(&bytes).is_err(), "{format:?}");
    }
}

#[test]
fn jpeg_marker_scanning_ignores_embedded_thumbnail_eoi_and_partial_footer_is_an_error() {
    let bytes = encode_editable(&original(), RasterFormat::Jpeg, &bundle()).unwrap();
    let start = bytes.windows(8).position(|w| w == b"SCAPDOC\0").unwrap();
    for extra in 1..8 {
        assert!(extract_editable(&bytes[..start + extra]).is_err());
    }
    // Add an APP15 segment containing fake SOI/EOI bytes; it is opaque metadata.
    // Use a plain file because inserting metadata intentionally breaks the binding.
    let mut plain = encode(&original(), RasterFormat::Jpeg).unwrap();
    plain.splice(2..2, [0xff, 0xef, 0x00, 0x06, 0xff, 0xd8, 0xff, 0xd9]);
    assert!(extract_editable(&plain).unwrap().is_none());
    assert!(open(&plain).is_ok());
}

#[test]
fn resource_order_is_canonical_and_empty_documents_are_supported() {
    let mut editable = bundle();
    let id = ResourceId::from("extra");
    editable.document.resources.push(Resource {
        id: id.clone(),
        media_type: "application/octet-stream".into(),
    });
    editable.resources.insert(
        0,
        ResourceData {
            id,
            bytes: vec![1, 2, 3],
        },
    );
    let normalized = EditableDocument::new(editable.document, editable.resources).unwrap();
    assert_eq!(normalized.resources[0].id.0, "original-background");
    for format in FORMATS {
        let bytes = encode_editable(&original(), format, &normalized).unwrap();
        assert_eq!(extract_editable(&bytes).unwrap(), Some(normalized.clone()));
        let empty = EditableDocument::new(Document::new(Size::new(24.0, 16.0)), vec![]).unwrap();
        let bytes = encode_editable(&original(), format, &empty).unwrap();
        assert_eq!(extract_editable(&bytes).unwrap(), Some(empty));
    }
}
