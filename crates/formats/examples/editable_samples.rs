//! Synthetic files for default-viewer compatibility checks (M1-8).

use std::error::Error;
use std::path::PathBuf;

use scribblecap_core::render::{BackgroundImage, render};
use scribblecap_core::{
    Arrow, Background, Color, Document, Element, ElementId, Point, Resource, ResourceId, Shape,
    Size,
};
use scribblecap_formats::{
    EditableDocument, RasterFormat, RasterImage, ResourceData, encode, encode_editable, open,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/editable-samples"));
    std::fs::create_dir_all(&output)?;
    let (width, height) = (320, 200);
    let pixels: Vec<u8> = (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| [210 + (x / 8) as u8, 210 + (y / 8) as u8, 245, 255])
        })
        .collect();
    let original = RasterImage::new(width, height, pixels)?;
    let id = ResourceId::from("synthetic-original");
    let mut document = Document::new(Size::new(f64::from(width), f64::from(height)));
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
            start: Point::new(50.0, 155.0),
            end: Point::new(270.0, 45.0),
            color: Color::rgb(225, 45, 65),
            width: 20.0,
            head_at_start: false,
        }),
    });
    let background = BackgroundImage::new(&id, width, height, original.pixels())?;
    let flat = render(&document, Some(background))?;
    let preview = RasterImage::new(flat.width(), flat.height(), flat.into_pixels())?;
    let editable = EditableDocument::new(
        document,
        vec![ResourceData {
            id,
            bytes: encode(&original, RasterFormat::Png)?,
        }],
    )?;
    for (extension, format) in [
        ("png", RasterFormat::Png),
        ("jpg", RasterFormat::Jpeg),
        ("webp", RasterFormat::WebP),
    ] {
        let bytes = encode_editable(&preview, format, &editable)?;
        assert_eq!(open(&bytes)?.editable.as_ref(), Some(&editable));
        let path = output.join(format!("editable-example.{extension}"));
        std::fs::write(&path, bytes)?;
        println!("{}", path.display());
    }
    Ok(())
}
