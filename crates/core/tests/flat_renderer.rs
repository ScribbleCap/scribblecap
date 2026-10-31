use scribblecap_core::render::{BackgroundImage, RenderError, render};
use scribblecap_core::*;

fn doc(shapes: Vec<Shape>) -> Document {
    let mut document = Document::new(Size::new(192.0, 144.0));
    document.elements = shapes
        .into_iter()
        .enumerate()
        .map(|(i, shape)| Element {
            id: ElementId(format!("element-{i}")),
            shape,
        })
        .collect();
    document
}

fn rectangle(rect: Rect, fill: Color) -> Shape {
    Shape::Rectangle(Rectangle {
        rect,
        fill: Some(fill),
        stroke: fill,
        width: 2.0,
        corner_radius: 0.0,
    })
}

fn pixel(image: &scribblecap_core::render::RenderedImage, x: usize, y: usize) -> &[u8] {
    let i = (y * image.width() as usize + x) * 4;
    &image.pixels()[i..i + 4]
}

fn cases() -> Vec<(&'static str, Document)> {
    let red = Color::rgb(225, 45, 65);
    let blue = Color::rgb(25, 110, 230);
    let mut rounded = if let Shape::Rectangle(r) = rectangle(
        Rect::new(85.5, 20.5, 85.0, 48.0),
        Color::rgba(40, 180, 120, 160),
    ) {
        r
    } else {
        unreachable!()
    };
    rounded.corner_radius = 16.0;
    rounded.stroke = Color::rgb(0, 90, 55);
    rounded.width = 4.0;
    let mut cases = vec![
        (
            "arrows",
            doc(vec![
                Shape::Arrow(Arrow {
                    start: Point::new(12.0, 25.0),
                    end: Point::new(174.0, 51.0),
                    color: red,
                    width: 16.0,
                    head_at_start: false,
                }),
                Shape::Arrow(Arrow {
                    start: Point::new(20.0, 108.0),
                    end: Point::new(167.0, 87.0),
                    color: blue,
                    width: 12.0,
                    head_at_start: true,
                }),
            ]),
        ),
        (
            "shapes",
            doc(vec![
                rectangle(Rect::new(-8.0, 12.0, 73.0, 58.0), red),
                Shape::Rectangle(rounded),
                Shape::Ellipse(Ellipse {
                    rect: Rect::new(24.0, 86.0, 70.0, 38.0),
                    fill: Some(Color::rgba(120, 70, 200, 120)),
                    stroke: blue,
                    width: 5.0,
                }),
                Shape::Line(Line {
                    start: Point::new(113.0, 91.0),
                    end: Point::new(184.0, 132.0),
                    color: Color::rgba(0, 130, 100, 170),
                    width: 8.0,
                }),
            ]),
        ),
        (
            "freehand",
            doc(vec![
                Shape::Freehand(Freehand {
                    tool: FreehandTool::Marker,
                    color: blue,
                    width: 7.0,
                    points: vec![
                        Point::new(12.0, 35.0),
                        Point::new(35.0, 15.0),
                        Point::new(61.0, 43.0),
                        Point::new(90.0, 19.0),
                        Point::new(120.0, 40.0),
                        Point::new(160.0, 23.0),
                    ],
                }),
                Shape::Freehand(Freehand {
                    tool: FreehandTool::Highlighter,
                    color: Color::rgba(255, 230, 0, 102),
                    width: 24.0,
                    points: vec![
                        Point::new(18.0, 82.0),
                        Point::new(166.0, 82.0),
                        Point::new(70.0, 117.0),
                        Point::new(70.0, 65.0),
                    ],
                }),
                Shape::Freehand(Freehand {
                    tool: FreehandTool::Marker,
                    color: red,
                    width: 10.0,
                    points: vec![Point::new(176.0, 120.0)],
                }),
            ]),
        ),
        (
            "stamps",
            doc([
                "approved",
                "rejected",
                "question",
                "exclaim",
                "perfect",
                "future-stamp",
            ]
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                Shape::Stamp(Stamp {
                    center: Point::new(33.0 + (i % 3) as f64 * 62.0, 33.0 + (i / 3) as f64 * 70.0),
                    stamp: name.into(),
                    diameter: 42.0,
                    tail_angle: Some(i as f64 * 67.0),
                })
            })
            .collect()),
        ),
    ];
    for (name, style) in [
        ("text_plain", TextStyle::Plain),
        ("text_bubble", TextStyle::Bubble),
        ("text_label", TextStyle::Label),
    ] {
        let mut document = doc(vec![rectangle(
            Rect::new(0.0, 0.0, 192.0, 144.0),
            Color::rgb(225, 232, 240),
        )]);
        document.elements.push(Element {
            id: ElementId::from("text"),
            shape: Shape::Text(Text {
                origin: Point::new(15.0, 12.0),
                text: "Office café\nClick here first!".into(),
                style,
                color: Color::rgb(20, 60, 120),
                font_size: 22.0,
                font: None,
                wrap_width: Some(150.0),
                align: TextAlign::Center,
                attached_to: (style == TextStyle::Label).then(|| ElementId::from("element-0")),
            }),
        });
        cases.push((name, document));
    }
    cases.push((
        "steps",
        doc([1, 12, 999, u32::MAX]
            .into_iter()
            .enumerate()
            .map(|(i, number)| {
                Shape::Step(Step {
                    center: Point::new(49.0 + (i % 2) as f64 * 94.0, 35.0 + (i / 2) as f64 * 73.0),
                    number,
                    color: if i % 2 == 0 {
                        blue
                    } else {
                        Color::rgb(255, 220, 80)
                    },
                    diameter: 60.0,
                })
            })
            .collect()),
    ));
    let mut typography = Document::new(Size::new(380.0, 240.0));
    for (i, (size, content)) in [
        (12.0, "AV office café"),
        (16.0, "Ελληνικά Русский"),
        (24.0, "naïve e\u{301} ﬁ"),
        (36.0, "White text"),
        (48.0, "Scribble"),
    ]
    .into_iter()
    .enumerate()
    {
        typography.elements.push(Element {
            id: ElementId(format!("type-{i}")),
            shape: Shape::Text(Text {
                origin: Point::new(12.5, [4.0, 28.0, 57.0, 95.0, 154.0][i]),
                text: content.into(),
                style: TextStyle::Plain,
                color: if i == 3 {
                    Color::rgb(255, 255, 255)
                } else {
                    Color::rgba(30, 85, 160, 190)
                },
                font_size: size,
                font: None,
                wrap_width: None,
                align: TextAlign::Left,
                attached_to: None,
            }),
        });
    }
    cases.push(("typography", typography));
    let mut composite = doc(vec![
        rectangle(
            Rect::new(-5.0, 20.0, 130.0, 50.0),
            Color::rgba(255, 100, 30, 120),
        ),
        Shape::Ellipse(Ellipse {
            rect: Rect::new(64.0, 42.0, 105.0, 80.0),
            fill: Some(Color::rgba(30, 100, 255, 140)),
            stroke: Color::rgba(0, 30, 100, 200),
            width: 5.0,
        }),
    ]);
    let id = ResourceId::from("gradient");
    composite.resources.push(Resource {
        id: id.clone(),
        media_type: "image/png".into(),
    });
    composite.background = Some(Background {
        resource: id,
        origin: Point::new(8.5, 9.5),
    });
    cases.push(("composite", composite));
    cases
}

#[test]
fn golden_images_match_exact_rgba_pixels() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/render");
    for (name, document) in cases() {
        let pixels: Vec<u8> = (0..100)
            .flat_map(|y| (0..120).flat_map(move |x| [x as u8 * 2, y as u8 * 2, 100, 180]))
            .collect();
        let background = document
            .background
            .as_ref()
            .map(|bg| BackgroundImage::new(&bg.resource, 120, 100, &pixels).unwrap());
        let image = render(&document, background).unwrap();
        let file = directory.join(format!("{name}.png"));
        if std::env::var_os("SCRIBBLECAP_UPDATE_GOLDEN").is_some() {
            std::fs::create_dir_all(&directory).unwrap();
            let mut pixmap = tiny_skia::Pixmap::new(image.width(), image.height()).unwrap();
            for (src, dst) in image
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(pixmap.data_mut().as_chunks_mut::<4>().0.iter_mut())
            {
                let a = u16::from(src[3]);
                for i in 0..3 {
                    dst[i] = ((u16::from(src[i]) * a + 127) / 255) as u8;
                }
                dst[3] = src[3];
            }
            pixmap.save_png(&file).unwrap();
        }
        let expected =
            tiny_skia::Pixmap::load_png(&file).expect("golden missing; regenerate and review it");
        assert_eq!(
            (image.width(), image.height()),
            (expected.width(), expected.height())
        );
        assert_eq!(
            image.pixels(),
            expected.take_demultiplied(),
            "golden changed: {name}"
        );
    }
}

#[test]
fn transparent_canvas_and_fractional_dimensions() {
    let image = render(&Document::new(Size::new(2.2, 1.1)), None).unwrap();
    assert_eq!((image.width(), image.height()), (3, 2));
    assert!(image.pixels().iter().all(|v| *v == 0));
}

#[test]
fn document_order_alpha_and_clipping() {
    let document = doc(vec![
        rectangle(Rect::new(-10.0, -10.0, 60.0, 60.0), Color::rgb(0, 0, 255)),
        rectangle(
            Rect::new(10.0, 10.0, 60.0, 60.0),
            Color::rgba(255, 0, 0, 128),
        ),
    ]);
    let image = render(&document, None).unwrap();
    assert_eq!(pixel(&image, 0, 0), &[0, 0, 255, 255]);
    assert_eq!(pixel(&image, 20, 20), &[128, 0, 127, 255]);
    assert_eq!(pixel(&image, 60, 60), &[255, 0, 0, 128]);
    assert_eq!(pixel(&image, 100, 100), &[0, 0, 0, 0]);
}

#[test]
fn background_uses_resource_identity_origin_and_source_over() {
    let mut document = doc(vec![rectangle(
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Color::rgba(255, 0, 0, 128),
    )]);
    let resource = ResourceId::from("bg");
    document.resources.push(Resource {
        id: resource.clone(),
        media_type: "image/png".into(),
    });
    document.background = Some(Background {
        resource: resource.clone(),
        origin: Point::new(-1.0, 5.0),
    });
    let pixels = [0, 0, 255, 255].repeat(6);
    let bg = BackgroundImage::new(&resource, 3, 2, &pixels).unwrap();
    let image = render(&document, Some(bg)).unwrap();
    assert_eq!(pixel(&image, 1, 5), &[128, 0, 127, 255]);
    assert_eq!(pixel(&image, 2, 5), &[255, 0, 0, 128]);
    assert_eq!(pixel(&image, 10, 3), &[255, 0, 0, 128]);
    assert!(matches!(
        render(&document, None),
        Err(RenderError::MissingBackground(_))
    ));
    let wrong = ResourceId::from("wrong");
    let bg = BackgroundImage::new(&wrong, 3, 2, &pixels).unwrap();
    assert!(matches!(
        render(&document, Some(bg)),
        Err(RenderError::MissingBackground(_))
    ));
}

#[test]
fn highlighter_self_intersection_is_one_paint_operation() {
    let document = doc(vec![Shape::Freehand(Freehand {
        tool: FreehandTool::Highlighter,
        color: Color::rgba(255, 230, 0, 102),
        width: 20.0,
        points: vec![
            Point::new(10.0, 40.0),
            Point::new(100.0, 40.0),
            Point::new(10.0, 40.0),
        ],
    })]);
    let image = render(&document, None).unwrap();
    assert_eq!(pixel(&image, 50, 40)[3], 102);
}

#[test]
fn rejects_invalid_or_excessive_inputs() {
    assert!(render(&Document::new(Size::new(f64::NAN, 10.0)), None).is_err());
    assert!(render(&Document::new(Size::new(1e20, 1e20)), None).is_err());
    assert!(render(&Document::new(Size::new(100_000.0, 100_000.0)), None).is_err());
    let id = ResourceId::from("bg");
    assert!(BackgroundImage::new(&id, 2, 2, &[0; 15]).is_err());
    assert!(BackgroundImage::new(&id, 0, 2, &[]).is_err());
}

#[test]
fn degenerate_shapes_are_empty_and_single_point_strokes_are_dots() {
    let red = Color::rgb(255, 0, 0);
    let empty = doc(vec![
        Shape::Arrow(Arrow {
            start: Point::new(20.0, 20.0),
            end: Point::new(20.0, 20.0),
            width: 10.0,
            color: red,
            head_at_start: false,
        }),
        rectangle(Rect::new(20.0, 20.0, 0.0, 10.0), red),
        Shape::Ellipse(Ellipse {
            rect: Rect::new(20.0, 20.0, 10.0, 0.0),
            fill: Some(red),
            stroke: red,
            width: 2.0,
        }),
    ]);
    assert!(
        render(&empty, None)
            .unwrap()
            .pixels()
            .iter()
            .all(|v| *v == 0)
    );
    let dot = doc(vec![Shape::Line(Line {
        start: Point::new(30.0, 30.0),
        end: Point::new(30.0, 30.0),
        color: red,
        width: 10.0,
    })]);
    let image = render(&dot, None).unwrap();
    assert_eq!(pixel(&image, 30, 30), &[255, 0, 0, 255]);
    assert_eq!(pixel(&image, 36, 30), &[0, 0, 0, 0]);
}

#[test]
fn oversized_corner_radius_is_clamped_and_unfilled_shapes_have_no_interior() {
    let mut r = Rectangle {
        rect: Rect::new(10.0, 10.0, 50.0, 30.0),
        stroke: Color::rgb(0, 0, 0),
        fill: None,
        width: 2.0,
        corner_radius: 1000.0,
    };
    let image = render(&doc(vec![Shape::Rectangle(r.clone())]), None).unwrap();
    r.corner_radius = 15.0;
    assert_eq!(
        image,
        render(&doc(vec![Shape::Rectangle(r)]), None).unwrap()
    );
    assert_eq!(pixel(&image, 30, 25), &[0, 0, 0, 0]);
}

#[test]
fn background_premultiplication_does_not_leak_hidden_rgb() {
    let mut document = Document::new(Size::new(2.0, 1.0));
    let id = ResourceId::from("bg");
    document.resources.push(Resource {
        id: id.clone(),
        media_type: "image/png".into(),
    });
    document.background = Some(Background {
        resource: id.clone(),
        origin: Point::new(0.0, 0.0),
    });
    let data = [255, 0, 100, 0, 255, 0, 0, 128];
    let background = BackgroundImage::new(&id, 2, 1, &data).unwrap();
    let image = render(&document, Some(background)).unwrap();
    assert_eq!(image.pixels(), &[0, 0, 0, 0, 255, 0, 0, 128]);
    assert!(matches!(
        render(&Document::new(Size::new(2.0, 1.0)), Some(background)),
        Err(RenderError::UnexpectedBackground)
    ));
}

#[test]
fn stamp_tail_geometry_normalizes_rotation_without_system_fonts() {
    use scribblecap_core::geometry::shape_paths;
    let stamp = |angle| {
        Shape::Stamp(Stamp {
            center: Point::new(40.0, 40.0),
            diameter: 20.0,
            stamp: "approved".into(),
            tail_angle: Some(angle),
        })
    };
    assert_eq!(
        shape_paths(&stamp(0.0)).unwrap(),
        shape_paths(&stamp(360.0)).unwrap()
    );
    assert_eq!(
        shape_paths(&stamp(-90.0)).unwrap(),
        shape_paths(&stamp(270.0)).unwrap()
    );
    let tail = shape_paths(&stamp(90.0)).unwrap().remove(0).path;
    assert!(
        tail.points()
            .any(|p| (p.x - 40.0).abs() < 1e-10 && (p.y - 55.0).abs() < 1e-10)
    );
}

#[test]
fn unrepresentable_geometry_returns_an_error() {
    let document = doc(vec![Shape::Arrow(Arrow {
        start: Point::new(0.0, 0.0),
        end: Point::new(1e300, 20.0),
        color: Color::rgb(255, 0, 0),
        width: 10.0,
        head_at_start: false,
    })]);
    assert!(matches!(
        render(&document, None),
        Err(RenderError::InvalidGeometry(_))
    ));
    let document = doc(vec![Shape::Line(Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(100.0, 20.0),
        color: Color::rgb(255, 0, 0),
        width: f64::MAX,
    })]);
    assert!(matches!(
        render(&document, None),
        Err(RenderError::InvalidGeometry(_))
    ));
}
