//! Undo/redo as a stack of whole-document snapshots. Layers are `Arc`-shared, so a
//! snapshot only costs the memory of the layers modified after it was taken.

use crate::document::Document;
use crate::selection::Selection;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub doc: Document,
    pub selection: Option<Selection>,
}

#[derive(Debug)]
pub struct History {
    undo: VecDeque<(State, usize)>,
    redo: Vec<State>,
    /// Redo stack cleared by the last push, restored if that push is discarded.
    cleared_redo: Vec<State>,
    total: usize,
    budget: usize,
    max_steps: usize,
}

impl Default for History {
    fn default() -> Self {
        History::new(512 * 1024 * 1024, 100)
    }
}

/// Bytes of `s` not shared with `prev`.
fn unique_cost(s: &State, prev: Option<&State>) -> usize {
    let sel = s.selection.as_ref().map_or(0, Selection::byte_size);
    let layers: usize = s
        .doc
        .layers
        .iter()
        .filter(|l| prev.is_none_or(|p| !p.doc.layers.iter().any(|q| Arc::ptr_eq(&q.image, &l.image))))
        .map(|l| l.image.byte_size())
        .sum();
    sel + layers
}

impl History {
    pub fn new(budget: usize, max_steps: usize) -> Self {
        History { undo: VecDeque::new(), redo: Vec::new(), cleared_redo: Vec::new(), total: 0, budget, max_steps: max_steps.max(1) }
    }

    /// Records the state *before* a modification. Clears the redo stack.
    pub fn push(&mut self, s: State) {
        let cost = unique_cost(&s, self.undo.back().map(|(s, _)| s));
        self.total += cost;
        self.undo.push_back((s, cost));
        self.cleared_redo = std::mem::take(&mut self.redo);
        while self.undo.len() > 1 && (self.total > self.budget || self.undo.len() > self.max_steps) {
            if let Some((_, c)) = self.undo.pop_front() {
                self.total -= c;
            }
        }
    }

    pub fn undo(&mut self, current: State) -> Option<State> {
        self.cleared_redo.clear();
        let (prev, cost) = self.undo.pop_back()?;
        self.total -= cost;
        self.redo.push(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: State) -> Option<State> {
        self.cleared_redo.clear();
        let next = self.redo.pop()?;
        let cost = unique_cost(&current, self.undo.back().map(|(s, _)| s));
        self.total += cost;
        self.undo.push_back((current, cost));
        Some(next)
    }

    /// Drops the last recorded state (an operation that changed nothing in the end).
    /// The redo stack it had cleared comes back.
    pub fn discard_last(&mut self) -> Option<State> {
        let (s, cost) = self.undo.pop_back()?;
        self.total -= cost;
        self.redo = std::mem::take(&mut self.cleared_redo);
        Some(s)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.cleared_redo.clear();
        self.total = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    fn state(c: Rgba) -> State {
        State { doc: Document::new(10, 10, c), selection: None }
    }

    #[test]
    fn undo_redo_cycle() {
        let mut h = History::default();
        assert!(!h.can_undo());
        h.push(state(Rgba::WHITE));
        let cur = state(Rgba::BLACK);
        let prev = h.undo(cur.clone()).unwrap();
        assert_eq!(prev.doc.active_image().get(0, 0), Rgba::WHITE);
        assert!(h.can_redo());
        let next = h.redo(prev).unwrap();
        assert_eq!(next, cur);
        assert!(h.can_undo() && !h.can_redo());
        h.push(state(Rgba::BLACK));
        assert!(!h.can_redo());
        assert!(h.discard_last().is_some());
        assert_eq!(h.undo_len(), 1);
        h.clear();
        assert!(!h.can_undo());
    }

    #[test]
    fn budget_and_step_limits() {
        let one = 10 * 10 * 4;
        let mut h = History::new(one * 3, 100);
        for _ in 0..10 {
            h.push(state(Rgba::WHITE));
        }
        assert_eq!(h.undo_len(), 3);
        let mut h = History::new(usize::MAX, 4);
        for _ in 0..10 {
            h.push(state(Rgba::WHITE));
        }
        assert_eq!(h.undo_len(), 4);
    }

    #[test]
    fn shared_layers_cost_nothing() {
        let mut h = History::new(10 * 10 * 4, 100);
        let s = state(Rgba::WHITE);
        for _ in 0..20 {
            h.push(s.clone());
        }
        assert_eq!(h.undo_len(), 20);
    }
}
