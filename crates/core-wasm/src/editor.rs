//! The editor's document session: JSON requests in, JSON responses out.
//!
//! Safe Rust with no WebAssembly specifics, so it is tested natively. The protocol
//! is documented in `docs/editor.md`.

use scribblecap_core::geometry::{PaintStyle, shape_paths};
use scribblecap_core::{Document, Element, ElementId, History, Shape, Size};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One open document, its undo history and the id counter for new elements.
pub struct Editor {
    document: Document,
    history: History,
    next_id: u64,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Request {
    /// Start a new document of the given canvas size (the background image size).
    Reset { canvas: Size },
    /// Session-only background identity, tracked by the ordinary undo history.
    SetBackgroundRevision { revision: u32 },
    /// Append a new element on top; responds with its id and paths.
    Add { shape: Shape },
    /// Replace an element's shape, keeping its id and stacking position.
    Update { id: ElementId, shape: Shape },
    /// Delete an element.
    Remove { id: ElementId },
    /// Paths for every element, bottom-most first.
    Paint,
    FilterPoints {
        points: Vec<scribblecap_core::Point>,
    },
    /// The document as saved JSON (`docs/file-format.md`).
    Document,
    /// End an undoable step: record the document if it changed since the last step.
    Commit,
    /// Go back one step (uncommitted changes count as a step).
    Undo,
    /// Go forward one step after an undo.
    Redo,
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
        let document = Document::new(Size::new(1.0, 1.0));
        Self {
            history: History::new(document.clone()),
            document,
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
            Request::SetBackgroundRevision { revision } => {
                let id = scribblecap_core::ResourceId(format!("session-background-{revision}"));
                self.document.background = Some(scribblecap_core::Background {
                    resource: id.clone(),
                    origin: scribblecap_core::Point::new(0.0, 0.0),
                });
                self.document.resources = vec![scribblecap_core::Resource {
                    id,
                    media_type: "image/png".into(),
                }];
                Ok(Value::Null)
            }
            Request::FilterPoints { points } => {
                if points.is_empty()
                    || points.len() > 8192
                    || points.iter().any(|p| {
                        !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e9 || p.y.abs() > 1e9
                    })
                {
                    return Err("Invalid stroke points".into());
                }
                Ok(json!(scribblecap_core::geometry::filter_points(&points)))
            }
            Request::Reset { canvas } => {
                let document = Document::new(canvas);
                document.validate().map_err(|e| e.to_string())?;
                self.history = History::new(document.clone());
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
            Request::Commit => {
                let changed = self.history.commit(&self.document);
                Ok(
                    json!({ "changed": changed, "canUndo": self.history.can_undo(), "canRedo": self.history.can_redo() }),
                )
            }
            Request::Undo => {
                let state = self.history.undo(&self.document);
                self.restore(state)
            }
            Request::Redo => {
                let state = self.history.redo();
                self.restore(state)
            }
            Request::Document => {
                // Runtime background identities participate in undo, never in output.
                let mut document = self.document.clone();
                document.background = None;
                document.resources.clear();
                document
                    .to_json()
                    .map(Value::String)
                    .map_err(|e| e.to_string())
            }
        }
    }

    /// Show a state from the history. Responds with every element (id, shape and
    /// paths, bottom-most first), or `elements: null` when there was no step.
    fn restore(&mut self, state: Option<Document>) -> Result<Value, String> {
        let elements = match state {
            Some(document) => {
                self.document = document;
                let mut out = Vec::with_capacity(self.document.elements.len());
                for element in &self.document.elements {
                    out.push(json!({
                        "id": element.id,
                        "shape": element.shape,
                        "paths": paths(&element.shape)?,
                    }));
                }
                Value::Array(out)
            }
            None => Value::Null,
        };
        Ok(json!({
            "backgroundRevision": self.document.background.as_ref().and_then(|b| b.resource.0.strip_prefix("session-background-")).and_then(|r|r.parse::<u32>().ok()).unwrap_or(0),
            "elements": elements,
            "canUndo": self.history.can_undo(),
            "canRedo": self.history.can_redo(),
        }))
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
    fn background_revision_is_undoable_without_changing_annotations_or_saved_metadata() {
        let mut editor = Editor::default();
        ok(&mut editor, json!({"op":"reset","canvas":[200,100]}));
        ok(&mut editor, json!({"op":"add","shape":arrow()}));
        ok(&mut editor, json!({"op":"commit"}));
        ok(
            &mut editor,
            json!({"op":"setBackgroundRevision","revision":1}),
        );
        ok(&mut editor, json!({"op":"commit"}));
        let saved = ok(&mut editor, json!({"op":"document"}));
        let doc = Document::from_json(saved.as_str().unwrap()).unwrap();
        assert!(doc.background.is_none() && doc.resources.is_empty());
        assert_eq!(doc.elements.len(), 1);
        let undo = ok(&mut editor, json!({"op":"undo"}));
        assert_eq!(undo["backgroundRevision"], 0);
        assert_eq!(undo["elements"].as_array().unwrap().len(), 1);
        let empty = ok(&mut editor, json!({"op":"undo"}));
        assert_eq!(empty["elements"].as_array().unwrap().len(), 0);
        ok(&mut editor, json!({"op":"redo"}));
        assert_eq!(
            ok(&mut editor, json!({"op":"redo"}))["backgroundRevision"],
            1
        );
    }

    #[test]
    fn filter_points_is_bounded_and_uses_core_smoothing() {
        let mut editor = Editor::default();
        let points = ok(
            &mut editor,
            json!({"op":"filterPoints","points":[[0,0],[20,10],[40,0]]}),
        );
        assert!(points.as_array().unwrap().len() > 3);
        assert_eq!(
            points.as_array().unwrap().last().unwrap(),
            &json!([40.0, 0.0])
        );
        for points in [json!([]), json!([[1e100, 0]]), json!(vec![[0, 0]; 8193])] {
            assert!(
                editor
                    .handle(&json!({"op":"filterPoints","points":points}).to_string())
                    .contains("error")
            );
        }
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
    fn undo_and_redo_restore_shapes() {
        let mut editor = Editor::default();
        ok(&mut editor, json!({ "op": "reset", "canvas": [200, 100] }));
        let nothing = ok(&mut editor, json!({ "op": "undo" }));
        assert_eq!(nothing["elements"], Value::Null);

        ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        let step = ok(&mut editor, json!({ "op": "commit" }));
        assert_eq!(
            step,
            json!({ "changed": true, "canUndo": true, "canRedo": false })
        );
        let moved = json!({ "kind": "arrow", "start": [20, 20], "end": [130, 90], "color": "#e5194a", "width": 8 });
        ok(
            &mut editor,
            json!({ "op": "update", "id": "el-1", "shape": moved }),
        );
        ok(&mut editor, json!({ "op": "commit" }));
        let again = ok(&mut editor, json!({ "op": "commit" }));
        assert_eq!(again["changed"], false);

        let back = ok(&mut editor, json!({ "op": "undo" }));
        assert_eq!(back["elements"][0]["id"], "el-1");
        assert_eq!(back["elements"][0]["shape"]["start"], json!([10.0, 10.0]));
        assert!(back["elements"][0]["paths"][0]["d"].is_string());
        assert_eq!(back["canRedo"], true);
        let empty = ok(&mut editor, json!({ "op": "undo" }));
        assert_eq!(empty["elements"], json!([]));
        assert_eq!(empty["canUndo"], false);

        ok(&mut editor, json!({ "op": "redo" }));
        let forward = ok(&mut editor, json!({ "op": "redo" }));
        assert_eq!(
            forward["elements"][0]["shape"]["start"],
            json!([20.0, 20.0])
        );
        assert_eq!(forward["canRedo"], false);

        // New ids continue after undo, so a redo can never clash with a new element.
        ok(&mut editor, json!({ "op": "undo" }));
        let added = ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        assert_eq!(added["id"], "el-2");
    }

    #[test]
    fn reset_clears_history() {
        let mut editor = Editor::default();
        ok(&mut editor, json!({ "op": "reset", "canvas": [200, 100] }));
        ok(&mut editor, json!({ "op": "add", "shape": arrow() }));
        ok(&mut editor, json!({ "op": "commit" }));
        ok(&mut editor, json!({ "op": "reset", "canvas": [300, 100] }));
        let nothing = ok(&mut editor, json!({ "op": "undo" }));
        assert_eq!(nothing["elements"], Value::Null);
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
