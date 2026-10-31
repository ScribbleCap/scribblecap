use scribblecap_core::{geometry::shape_paths, *};

fn text(value: &str) -> Text {
    Text {
        origin: Point::new(10.0, 10.0),
        text: value.into(),
        style: TextStyle::Plain,
        color: Color::rgb(20, 40, 80),
        font_size: 24.0,
        font: None,
        wrap_width: None,
        align: TextAlign::Left,
        attached_to: None,
    }
}

#[test]
fn all_text_styles_and_step_numbers_have_shared_paths() {
    for style in [TextStyle::Plain, TextStyle::Bubble, TextStyle::Label] {
        let mut t = text("Office café\nΕλληνικά Русский");
        t.style = style;
        let paths = shape_paths(&Shape::Text(t)).unwrap();
        assert!(!paths.is_empty());
        assert!(
            paths
                .iter()
                .flat_map(|p| p.path.points())
                .all(|p| p.x.is_finite() && p.y.is_finite())
        );
    }
    for number in [1, 12, 999, u32::MAX] {
        assert!(
            !shape_paths(&Shape::Step(Step {
                center: Point::new(50.0, 50.0),
                number,
                color: Color::rgb(25, 110, 230),
                diameter: 40.0
            }))
            .unwrap()
            .is_empty()
        );
    }
}

#[test]
fn layout_wraps_aligns_and_preserves_empty_lines() {
    use scribblecap_core::geometry::layout_text;
    let one = layout_text(&text("hello")).unwrap();
    let three = layout_text(&text("hello\n\nworld")).unwrap();
    assert_eq!(three.height, one.height * 3.0);
    let mut t = text("hello world");
    t.wrap_width = Some(one.width + 1.0);
    let wrapped = layout_text(&t).unwrap();
    assert!(wrapped.height > one.height);
    t.text = "A".into();
    t.wrap_width = Some(100.0);
    let left = layout_text(&t).unwrap();
    t.align = TextAlign::Right;
    let right = layout_text(&t).unwrap();
    let x = |p: &Path| p.points().map(|p| p.x).fold(f64::INFINITY, f64::min);
    assert!(x(&right.path) > x(&left.path) + 70.0);
}

#[test]
fn fallback_font_is_deterministic_and_missing_glyphs_are_explicit() {
    let mut t = text("AV office e\u{301}");
    let default = shape_paths(&Shape::Text(t.clone())).unwrap();
    t.font = Some("A font that is not installed".into());
    assert_eq!(default, shape_paths(&Shape::Text(t)).unwrap());
    assert!(shape_paths(&Shape::Text(text("漢字"))).is_err());
    assert!(shape_paths(&Shape::Text(text(&"a".repeat(16_385)))).is_err());
    assert!(shape_paths(&Shape::Text(text(""))).unwrap().is_empty());
}

#[test]
fn narrow_wrap_keeps_graphemes_and_makes_progress() {
    use scribblecap_core::geometry::layout_text;
    let mut t = text("e\u{301}e\u{301}");
    t.wrap_width = Some(0.01);
    let wrapped = layout_text(&t).unwrap();
    assert_eq!(
        wrapped.height,
        layout_text(&text("é")).unwrap().height * 2.0
    );
    assert_eq!(
        layout_text(&text("a\r\nb")).unwrap(),
        layout_text(&text("a\nb")).unwrap()
    );
}

#[test]
fn shaping_applies_kerning_and_combining_marks() {
    use scribblecap_core::geometry::layout_text;
    let width = |value| layout_text(&text(value)).unwrap().width;
    assert!(width("AV") < width("A") + width("V"));
    assert_eq!(
        layout_text(&text("é")).unwrap(),
        layout_text(&text("e\u{301}")).unwrap()
    );
    assert_eq!(
        layout_text(&text("a\tb")).unwrap(),
        layout_text(&text("a    b")).unwrap()
    );
}

#[test]
fn missing_glyph_errors_name_the_annotation_and_huge_metrics_fail() {
    use scribblecap_core::render::{RenderError, render};
    let mut document = Document::new(Size::new(100.0, 100.0));
    document.elements.push(Element {
        id: ElementId::from("missing-glyph"),
        shape: Shape::Text(text("漢")),
    });
    match render(&document, None) {
        Err(RenderError::UnsupportedElement { element, .. }) => {
            assert_eq!(element.0, "missing-glyph")
        }
        other => panic!("expected explicit missing glyph error, got {other:?}"),
    }
    let mut huge = text("text");
    huge.font_size = f64::MAX;
    document.elements[0].shape = Shape::Text(huge);
    assert!(render(&document, None).is_err());
}

#[test]
fn multi_digit_step_ink_fits_inside_the_badge() {
    use scribblecap_core::geometry::PaintStyle;
    let center = Point::new(50.0, 50.0);
    let color = Color::rgb(255, 255, 100);
    let paths = shape_paths(&Shape::Step(Step {
        center,
        number: u32::MAX,
        color,
        diameter: 40.0,
    }))
    .unwrap();
    let ink = paths.last().unwrap();
    assert_eq!(ink.style, PaintStyle::Fill(Color::rgb(0, 0, 0)));
    for point in ink.path.points() {
        assert!((point.x - center.x).abs() <= 13.00001);
        assert!((point.y - center.y).abs() <= 11.00001);
    }
}
