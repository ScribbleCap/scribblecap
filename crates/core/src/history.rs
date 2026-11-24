//! Undo/redo history for a [`Document`].
//!
//! The history keeps whole document snapshots, one per committed step. A document
//! lists elements and resource ids only (image bytes live in the container), so a
//! snapshot is small; this keeps every operation undoable without a separate inverse
//! for each kind of edit.

use crate::Document;

/// Steps kept for undo; the oldest is dropped past this.
pub const MAX_STEPS: usize = 500;

/// Committed document states around the current one.
#[derive(Clone, Debug)]
pub struct History {
    /// The state as of the last commit (or undo/redo).
    current: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl History {
    /// A history whose starting point is `document`, with nothing to undo.
    pub fn new(document: Document) -> Self {
        Self {
            current: document,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Record `document` as a new step if it differs from the last committed state.
    /// A new step clears the redo stack. Returns whether a step was recorded.
    pub fn commit(&mut self, document: &Document) -> bool {
        if *document == self.current {
            return false;
        }
        let previous = std::mem::replace(&mut self.current, document.clone());
        self.undo.push(previous);
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    /// Step back. `document` (the live state) is committed first, so uncommitted
    /// changes are undone as one step rather than lost. Returns the state to show.
    pub fn undo(&mut self, document: &Document) -> Option<Document> {
        self.commit(document);
        let previous = self.undo.pop()?;
        self.redo
            .push(std::mem::replace(&mut self.current, previous.clone()));
        Some(previous)
    }

    /// Step forward again after [`History::undo`]. Returns the state to show.
    pub fn redo(&mut self) -> Option<Document> {
        let next = self.redo.pop()?;
        self.undo
            .push(std::mem::replace(&mut self.current, next.clone()));
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Element, ElementId, Line, Point, Shape, Size};

    fn with_lines(n: usize) -> Document {
        let mut document = Document::new(Size::new(100.0, 100.0));
        for i in 0..n {
            document.elements.push(Element {
                id: ElementId(format!("el-{i}")),
                shape: Shape::Line(Line {
                    start: Point::new(0.0, i as f64),
                    end: Point::new(50.0, i as f64),
                    color: Color::rgb(0, 0, 0),
                    width: 2.0,
                }),
            });
        }
        document
    }

    #[test]
    fn undo_and_redo_walk_the_steps() {
        let mut history = History::new(with_lines(0));
        assert!(history.commit(&with_lines(1)));
        assert!(history.commit(&with_lines(2)));
        assert_eq!(history.undo(&with_lines(2)), Some(with_lines(1)));
        assert_eq!(history.undo(&with_lines(1)), Some(with_lines(0)));
        assert_eq!(history.undo(&with_lines(0)), None);
        assert_eq!(history.redo(), Some(with_lines(1)));
        assert_eq!(history.redo(), Some(with_lines(2)));
        assert_eq!(history.redo(), None);
    }

    #[test]
    fn unchanged_commit_records_nothing() {
        let mut history = History::new(with_lines(1));
        assert!(!history.commit(&with_lines(1)));
        assert!(!history.can_undo());
    }

    #[test]
    fn new_step_clears_redo() {
        let mut history = History::new(with_lines(0));
        history.commit(&with_lines(1));
        history.undo(&with_lines(1));
        assert!(history.can_redo());
        history.commit(&with_lines(3));
        assert!(!history.can_redo());
        assert_eq!(history.undo(&with_lines(3)), Some(with_lines(0)));
    }

    #[test]
    fn undo_includes_uncommitted_changes() {
        let mut history = History::new(with_lines(0));
        assert_eq!(history.undo(&with_lines(2)), Some(with_lines(0)));
        assert_eq!(history.redo(), Some(with_lines(2)));
    }

    #[test]
    fn oldest_steps_are_dropped() {
        let mut history = History::new(with_lines(0));
        let mut live = with_lines(0);
        for i in 1..=MAX_STEPS + 5 {
            live.canvas = Size::new(100.0 + i as f64, 100.0);
            history.commit(&live);
        }
        let mut count = 0;
        while let Some(previous) = history.undo(&live) {
            live = previous;
            count += 1;
        }
        assert_eq!(count, MAX_STEPS);
    }
}
