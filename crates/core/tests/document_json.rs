//! JSON round-trip and validation tests for the document format
//! (`docs/file-format.md`).

use scribblecap_core::*;

const RED: Color = Color::rgb(0xFF, 0x00, 0x00);
const BLUE: Color = Color::rgb(0x19, 0x77, 0xFF);

fn el(id: &str, shape: Shape) -> Element {
    Element {
        id: id.into(),
        shape,
    }
}

/// One element of every kind, using every optional key at least once.
fn every_kind() -> Vec<Element> {
    let p = Point::new;
    vec![
        el(
            "arrow",
            Shape::Arrow(Arrow {
                start: p(10.0, 20.0),
                end: p(300.25, 40.5),
                color: RED,
                width: 12.0,
                head_at_start: false,
            }),
        ),
        el(
            "arrow-both",
            Shape::Arrow(Arrow {
                start: p(0.1, 0.2),
                end: p(1e4, 3.0),
                color: BLUE,
                width: 4.0,
                head_at_start: true,
            }),
        ),
        el(
            "line",
            Shape::Line(Line {
                start: p(-5.0, 2.0),
                end: p(5.0, 2.0),
                color: Color::rgb(0, 0, 0),
                width: 1.5,
            }),
        ),
        el(
            "rect",
            Shape::Rectangle(Rectangle {
                rect: Rect::new(1.0, 2.0, 3.0, 4.0),
                stroke: RED,
                fill: None,
                width: 6.0,
                corner_radius: 0.0,
            }),
        ),
        el(
            "rounded",
            Shape::Rectangle(Rectangle {
                rect: Rect::new(10.0, 20.0, 300.0, 0.0),
                stroke: RED,
                fill: Some(Color::rgba(255, 247, 0, 102)),
                width: 6.0,
                corner_radius: 20.0,
            }),
        ),
        el(
            "ellipse",
            Shape::Ellipse(Ellipse {
                rect: Rect::new(50.0, 60.0, 70.0, 80.0),
                stroke: BLUE,
                fill: Some(Color::rgb(255, 255, 255)),
                width: 8.0,
            }),
        ),
        el(
            "marker",
            Shape::Freehand(Freehand {
                tool: FreehandTool::Marker,
                color: RED,
                width: 6.0,
                points: vec![p(0.0, 0.0), p(1.0 / 3.0, 2.0 / 3.0), p(10.0, 0.1 + 0.2)],
            }),
        ),
        el(
            "highlighter",
            Shape::Freehand(Freehand {
                tool: FreehandTool::Highlighter,
                color: Color::rgba(255, 247, 0, 102),
                width: 24.0,
                points: vec![p(5.0, 5.0)],
            }),
        ),
        el(
            "text",
            Shape::Text(Text {
                origin: p(15.0, 25.0),
                text: "Hello\nwörld ✏️ \"quoted\"".into(),
                style: TextStyle::Plain,
                color: RED,
                font_size: 24.0,
                font: None,
                wrap_width: None,
                align: TextAlign::Left,
                attached_to: None,
            }),
        ),
        el(
            "bubble",
            Shape::Text(Text {
                origin: p(100.0, 200.0),
                text: "Bubble".into(),
                style: TextStyle::Bubble,
                color: Color::rgb(0, 0, 0),
                font_size: 18.5,
                font: Some("Inter".into()),
                wrap_width: Some(240.0),
                align: TextAlign::Center,
                attached_to: None,
            }),
        ),
        el(
            "label",
            Shape::Text(Text {
                origin: p(1.0, 10.0),
                text: "Label".into(),
                style: TextStyle::Label,
                color: Color::rgb(255, 255, 255),
                font_size: 24.0,
                font: None,
                wrap_width: None,
                align: TextAlign::Right,
                attached_to: Some("rect".into()),
            }),
        ),
        el(
            "stamp",
            Shape::Stamp(Stamp {
                center: p(400.0, 300.0),
                stamp: "approved".into(),
                diameter: 100.0,
                tail_angle: Some(-45.0),
            }),
        ),
        el(
            "stamp-no-tail",
            Shape::Stamp(Stamp {
                center: p(40.0, 30.0),
                stamp: "question".into(),
                diameter: 56.0,
                tail_angle: None,
            }),
        ),
        el(
            "step",
            Shape::Step(Step {
                center: p(90.0, 110.0),
                number: 3,
                color: BLUE,
                diameter: 36.0,
            }),
        ),
    ]
}

fn full_document() -> Document {
    let id = ResourceId::from("6f1c0e2a-3b0d-4c5e-9a57-2d8f4b1e7c90");
    Document {
        canvas: Size::new(1280.0, 800.0),
        background: Some(Background {
            resource: id.clone(),
            origin: Point::new(16.0, 8.0),
        }),
        resources: vec![Resource {
            id,
            media_type: "image/png".into(),
        }],
        elements: every_kind(),
    }
}

fn json_value(doc: &Document) -> serde_json::Value {
    serde_json::from_str(&doc.to_json().unwrap()).unwrap()
}

#[test]
fn every_element_kind_round_trips() {
    for element in every_kind() {
        let mut doc = full_document();
        // Keep the label's target so the document stays valid.
        doc.elements.retain(|e| e.id.0 == "rect");
        if element.id.0 != "rect" {
            doc.elements.push(element.clone());
        }
        let json = doc.to_json().unwrap();
        let back = Document::from_json(&json).unwrap();
        assert_eq!(back, doc, "{} did not round-trip: {json}", element.id);
    }
}

#[test]
fn full_document_round_trips_and_is_stable() {
    let doc = full_document();
    let json = doc.to_json().unwrap();
    let back = Document::from_json(&json).unwrap();
    assert_eq!(back, doc);
    assert_eq!(
        back.to_json().unwrap(),
        json,
        "second save differs from the first"
    );
}

#[test]
fn empty_document_round_trips() {
    let doc = Document::new(Size::new(1.0, 1.0));
    let json = doc.to_json().unwrap();
    assert_eq!(
        json,
        r#"{"format":"scribblecap/1","canvas":{"width":1.0,"height":1.0},"elements":[]}"#
    );
    assert_eq!(Document::from_json(&json).unwrap(), doc);
}

#[test]
fn writes_the_documented_shape() {
    let v = json_value(&full_document());
    assert_eq!(v["format"], "scribblecap/1");
    assert_eq!(v["background"]["origin"], serde_json::json!([16.0, 8.0]));
    assert_eq!(v["resources"][0]["mediaType"], "image/png");

    let e = &v["elements"];
    assert_eq!(e[0]["kind"], "arrow");
    assert_eq!(e[0]["start"], serde_json::json!([10.0, 20.0]));
    assert_eq!(e[0]["color"], "#FF0000FF");
    assert!(e[0].get("headAtStart").is_none(), "default omitted");
    assert_eq!(e[1]["headAtStart"], true);
    assert!(e[3].get("cornerRadius").is_none());
    assert!(e[3].get("fill").is_none());
    assert_eq!(e[4]["kind"], "rectangle");
    assert_eq!(e[4]["cornerRadius"], 20.0);
    assert_eq!(e[7]["tool"], "highlighter");
    assert!(e[8].get("style").is_none(), "plain is the default");
    assert_eq!(e[9]["style"], "bubble");
    assert_eq!(e[9]["wrapWidth"], 240.0);
    assert_eq!(e[9]["align"], "center");
    assert_eq!(e[10]["attachedTo"], "rect");
    assert_eq!(e[11]["tailAngle"], -45.0);
    assert_eq!(e[13]["kind"], "step");
}

#[test]
fn documented_example_parses() {
    let spec = include_str!("../../../docs/file-format.md");
    let example = spec
        .split("## Example")
        .nth(1)
        .and_then(|s| s.split("```json").nth(1))
        .and_then(|s| s.split("```").next())
        .expect("docs/file-format.md has a json block under ## Example");
    let doc = Document::from_json(example).unwrap();
    assert_eq!(doc.elements.len(), 5);
    assert_eq!(Document::from_json(&doc.to_json().unwrap()).unwrap(), doc);
}

#[test]
fn reads_short_colours_and_ignores_unknown_keys() {
    let doc = Document::from_json(
        r##"{"format":"scribblecap/1","canvas":{"width":10,"height":10},"future":1,
            "elements":[{"id":"l","kind":"line","start":[0,0],"end":[1,1],
            "color":"#ff0000","width":2,"addedLater":true}]}"##,
    )
    .unwrap();
    let Shape::Line(line) = &doc.elements[0].shape else {
        panic!("expected a line")
    };
    assert_eq!(line.color, RED);
}

fn load_err(json: &str) -> LoadError {
    Document::from_json(json).expect_err("should be rejected")
}

#[test]
fn rejects_other_formats() {
    assert!(matches!(load_err("{}"), LoadError::NotADocument));
    assert!(matches!(
        load_err(r#"{"format":"skitch"}"#),
        LoadError::NotADocument
    ));
    assert!(matches!(load_err("[1]"), LoadError::NotADocument));
    assert!(matches!(load_err("not json"), LoadError::Json(_)));
    match load_err(r#"{"format":"scribblecap/2","whatever":true}"#) {
        LoadError::UnsupportedFormat(v) => assert_eq!(v, "scribblecap/2"),
        e => panic!("unexpected {e}"),
    }
}

#[test]
fn rejects_schema_violations() {
    let base = r#"{"format":"scribblecap/1","canvas":{"width":10,"height":10},"elements":["#;
    for bad in [
        r##"{"id":"x","kind":"hexagon"}"##,
        r##"{"id":"x","kind":"line","start":[0,0],"end":[1,1],"color":"red","width":2}"##,
        r##"{"id":"x","kind":"line","start":[0],"end":[1,1],"color":"#000000","width":2}"##,
        r##"{"id":"x","kind":"line","end":[1,1],"color":"#000000","width":2}"##,
        r##"{"id":"x","kind":"step","center":[0,0],"number":-1,"color":"#000000","diameter":2}"##,
        r##"{"kind":"line","start":[0,0],"end":[1,1],"color":"#000000","width":2}"##,
    ] {
        let json = format!("{base}{bad}]}}");
        assert!(
            matches!(load_err(&json), LoadError::Json(_)),
            "accepted {bad}"
        );
    }
}

fn invalid(doc: &Document) -> ValidationError {
    let err = doc.to_json().expect_err("should not save");
    // The same rule is enforced on load. NaN and infinity can't occur in a file (JSON
    // has no such numbers; serde writes them as `null`), so skip those.
    let mut v = serde_json::to_value(doc).unwrap();
    v["format"] = "scribblecap/1".into();
    let json = v.to_string();
    if !json.contains("null") {
        match Document::from_json(&json) {
            Err(LoadError::Invalid(e)) => assert_eq!(e, err),
            other => panic!("load accepted what save rejected: {other:?}"),
        }
    }
    err
}

#[test]
fn rejects_rule_violations() {
    let ok = full_document();
    assert!(ok.validate().is_ok());

    let mut d = ok.clone();
    d.canvas.width = 0.0;
    assert_eq!(invalid(&d), ValidationError::InvalidCanvas);

    let mut d = ok.clone();
    d.resources.clear();
    assert!(matches!(invalid(&d), ValidationError::UnknownResource(_)));

    let mut d = ok.clone();
    d.resources.push(d.resources[0].clone());
    assert!(matches!(invalid(&d), ValidationError::DuplicateResource(_)));

    let mut d = ok.clone();
    d.elements[1].id = "arrow".into();
    assert_eq!(invalid(&d), ValidationError::DuplicateId("arrow".into()));

    let mut d = ok.clone();
    d.elements[0].id = "".into();
    assert_eq!(invalid(&d), ValidationError::EmptyId);
}

#[test]
fn rejects_bad_values() {
    let field_of = |mutate: &dyn Fn(&mut Shape)| {
        let mut d = full_document();
        for e in &mut d.elements {
            mutate(&mut e.shape);
        }
        match invalid(&d) {
            ValidationError::InvalidValue { field, .. } => field,
            e => panic!("unexpected {e}"),
        }
    };
    assert_eq!(
        field_of(&|s| if let Shape::Arrow(a) = s {
            a.width = 0.0
        }),
        "width"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Line(l) = s {
            l.end.x = f64::NAN
        }),
        "end"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Rectangle(r) = s {
            r.rect.width = -1.0
        }),
        "rect"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Rectangle(r) = s {
            r.corner_radius = -2.0
        }),
        "cornerRadius"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Freehand(f) = s {
            f.points.clear()
        }),
        "points"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Text(t) = s {
            t.wrap_width = Some(0.0)
        }),
        "wrapWidth"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Stamp(st) = s {
            st.tail_angle = Some(f64::INFINITY)
        }),
        "tailAngle"
    );
    assert_eq!(
        field_of(&|s| if let Shape::Step(st) = s {
            st.number = 0
        }),
        "number"
    );
}

#[test]
fn labels_must_attach_to_another_non_text_element() {
    let label_target = |style: TextStyle, target: &str| {
        let mut d = full_document();
        for e in &mut d.elements {
            if let Shape::Text(t) = &mut e.shape
                && e.id.0 == "label"
            {
                t.style = style;
                t.attached_to = Some(target.into());
            }
        }
        d
    };
    assert!(label_target(TextStyle::Label, "stamp").validate().is_ok());
    for (style, target) in [
        (TextStyle::Label, "missing"),
        (TextStyle::Label, "label"),
        (TextStyle::Label, "bubble"),
        (TextStyle::Plain, "rect"),
    ] {
        assert!(
            matches!(
                invalid(&label_target(style, target)),
                ValidationError::InvalidAttachment { .. }
            ),
            "{style:?} -> {target} accepted"
        );
    }
}
