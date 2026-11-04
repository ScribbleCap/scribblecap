//! The editor's document session: JSON requests in, JSON responses out.
//!
//! Safe Rust with no WebAssembly specifics, so it is tested natively. The protocol
//! is documented in `docs/editor.md`.

use scribblecap_core::geometry::{PaintStyle, shape_paths};
use scribblecap_core::{Document, Element, ElementId, Shape, Size};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One open document plus the id counter for new elements.
pub struct Editor {
    document: Document,
    next_id: u64,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Request {
    /// Start a new document of the given canvas size (the background image size).
    Reset { canvas: Size },
    /// Append a new element on top; responds with its id and paths.
    Add { shape: Shape },
    /// Replace an element's shape, keeping its id and stacking position.
    Update { id: ElementId, shape: Shape },
    /// Delete an element.
    Remove { id: ElementId },
    /// Paths for every element, bottom-most first.
    Paint,
    /// The document as saved JSON (`docs/file-format.md`).
    Document,
}

/// A path ready for Canvas 2D: SVG path data plus how to paint it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePath {
    d: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    round: Option<bool>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            document: Document::new(Size::new(1.0, 1.0)),
            next_id: 1,
        }
    }
}

impl Editor {
    /// Handle one request. Always returns JSON: `{"ok": …}` or `{"error": "…"}`.
    /// A request that would leave the document invalid changes nothing.
    pub fn handle(&mut self, request: &str) -> String {
        let result = serde_json::from_str::<Request>(request)
            .map_err(|e| format!("bad request: {e}"))
            .and_then(|r| self.apply(r));
        match result {
            Ok(value) => json!({ "ok": value }).to_string(),
            Err(message) => json!({ "error": message }).to_string(),
        }
    }

    fn apply(&mut self, request: Request) -> Result<Value, String> {
        match request {
            Request::Reset { canvas } => {
                let document = Document::new(canvas);
                document.validate().map_err(|e| e.to_string())?;
                self.document = document;
                self.next_id = 1;
                Ok(Value::Null)
            }
            Request::Add { shape } => {
                let id = ElementId(format!("el-{}", self.next_id));
                let paths = self.change(|doc| {
                    doc.elements.push(Element {
                        id: id.clone(),
                        shape,
                    });
                    Ok(doc.elements.len() - 1)
                })?;
                self.next_id += 1;
                Ok(json!({ "id": id, "paths": paths }))
            }
            Request::Update { id, shape } => {
                let paths = self.change(|doc| {
                    let index = index_of(doc, &id)?;
                    doc.elements[index].shape = shape;
                    Ok(index)
                })?;
                Ok(json!({ "paths": paths }))
            }
            Request::Remove { id } => {
                let index = index_of(&self.document, &id)?;
                self.document.elements.remove(index);
                Ok(Value::Null)
            }
            Request::Paint => {
                let mut out = Vec::with_capacity(self.document.elements.len());
                for element in &self.document.elements {
                    out.push(json!({ "id": element.id, "paths": paths(&element.shape)? }));
                }
                Ok(Value::Array(out))
            }
            Request::Document => self
                .document
                .to_json()
                .map(Value::String)
                .map_err(|e| e.to_string()),
        }
    }

    /// Apply `edit` to a copy, validate it, compute the changed element's paths,
    /// and only then commit. Returns those paths.
    fn change(
        &mut self,
        edit: impl FnOnce(&mut Document) -> Result<usize, String>,
    ) -> Result<Vec<WirePath>, String> {
        let mut next = self.document.clone();
        let index = edit(&mut next)?;
        next.validate().map_err(|e| e.to_string())?;
        let paths = paths(&next.elements[index].shape)?;
        self.document = next;
        Ok(paths)
    }
}

fn index_of(document: &Document, id: &ElementId) -> Result<usize, String> {
    document
        .elements
        .iter()
        .position(|e| &e.id == id)
        .ok_or_else(|| format!("no element {id}"))
}

fn paths(shape: &Shape) -> Result<Vec<WirePath>, String> {
    let painted = shape_paths(shape).map_err(|e| e.to_string())?;
    Ok(painted
        .into_iter()
        .map(|p| {
            let d = p.path.to_svg();
            match p.style {
                PaintStyle::Fill(color) => WirePath {
                    d,
                    fill: Some(color.to_string()),
                    stroke: None,
                    width: None,
                    round: None,
                },
                PaintStyle::Stroke {
                    color,
                    width,
                    round,
                } => WirePath {
                    d,
                    fill: None,
                    stroke: Some(color.to_string()),
                    width: Some(width),
                    round: Some(round),
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(editor: &mut Editor, request: Value) -> Value {
        let response: Value = serde_json::from_str(&editor.handle(&request.to_string())).unwrap();
        assert!(response.get("error").is_none(), "{response}");
        response["ok"].clone()
    }

    fn arrow() -> Value {
        json!({ "kind": "arrow", "start": [10, 10], "end": [120, 80], "color": "#e5194a", "width": 8 })
    }

    #[test]
    fn add_update_remove_round_trip() {
        let mut editor = Editor::default();
        ok(&mut editor, json!({ "op": "reset", "canvas": [200, 100] }));
        let added = ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        assert_eq!(added["id"], "el-1");
        assert!(added["paths"][0]["fill"].is_string());
        assert!(added["paths"][0]["d"].as_str().unwrap().starts_with('M'));

        let rect = json!({ "kind": "rectangle", "rect": [5, 5, 50, 30], "stroke": "#e5194a", "width": 4, "cornerRadius": 6 });
        let second = ok(&mut editor, json!({ "op": "add", "shape": rect }));
        assert_eq!(second["id"], "el-2");
        assert_eq!(second["paths"][0]["width"], 4.0);

        let moved = json!({ "kind": "arrow", "start": [20, 20], "end": [130, 90], "color": "#e5194a", "width": 8 });
        ok(
            &mut editor,
            json!({ "op": "update", "id": "el-1", "shape": moved }),
        );
        let painted = ok(&mut editor, json!({ "op": "paint" }));
        assert_eq!(painted[0]["id"], "el-1");
        assert_eq!(painted[1]["id"], "el-2");

        ok(&mut editor, json!({ "op": "remove", "id": "el-1" }));
        let saved = ok(&mut editor, json!({ "op": "document" }));
        let document = Document::from_json(saved.as_str().unwrap()).unwrap();
        assert_eq!(document.elements.len(), 1);
        assert_eq!(document.elements[0].id, ElementId::from("el-2"));
    }

    #[test]
    fn invalid_edits_change_nothing() {
        let mut editor = Editor::default();
        ok(&mut editor, json!({ "op": "reset", "canvas": [200, 100] }));
        ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        let bad = json!({ "kind": "arrow", "start": [0, 0], "end": [1, 1], "color": "#e5194a", "width": -3 });
        let response =
            editor.handle(&json!({ "op": "update", "id": "el-1", "shape": bad }).to_string());
        assert!(response.contains("\"error\""), "{response}");
        let response = editor.handle(&json!({ "op": "remove", "id": "missing" }).to_string());
        assert!(response.contains("no element missing"), "{response}");
        let response = editor.handle("not json");
        assert!(response.contains("bad request"), "{response}");
        let painted = ok(&mut editor, json!({ "op": "paint" }));
        assert_eq!(painted.as_array().unwrap().len(), 1);
    }

    #[test]
    fn ids_are_not_reused_after_remove() {
        let mut editor = Editor::default();
        ok(&mut editor, json!({ "op": "reset", "canvas": [200, 100] }));
        ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        ok(&mut editor, json!({ "op": "remove", "id": "el-1" }));
        let again = ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        assert_eq!(again["id"], "el-2");
    }
}
