//! Undo for the graphics editors: a step is the whole state before and after a stroke, fill or paste. What these
//! editors hold is small (a tileset is at most 1024 tiles of 64 bytes, with its Map16 and objects), so a snapshot is
//! cheaper to get right than an inverse for every operation.

/// The most steps kept; the oldest goes first.
const LIMIT: usize = 200;

struct Step<T> {
    name: String,
    before: T,
    after: T,
}

pub struct History<T> {
    undo: Vec<Step<T>>,
    redo: Vec<Step<T>>,
    /// The state when the step in progress began, and its name.
    open: Option<(String, T)>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), open: None }
    }
}

impl<T: Clone + PartialEq> History<T> {
    /// Starts a step from `state`, unless one is already open: a drag's later frames carry on its first one's step.
    pub fn begin(&mut self, name: &str, state: &T) {
        if self.open.is_none() {
            self.open = Some((name.to_owned(), state.clone()));
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Ends the open step at `state`. A step that changed nothing is dropped.
    pub fn commit(&mut self, state: &T) {
        let Some((name, before)) = self.open.take() else { return };
        if before == *state {
            return;
        }
        self.redo.clear();
        self.undo.push(Step { name, before, after: state.clone() });
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// One edit, begun and ended at once.
    pub fn record(&mut self, name: &str, before: T, after: &T) {
        self.open = Some((name.to_owned(), before));
        self.commit(after);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The names of the steps to undo, latest first, and to redo, next first.
    pub fn names(&self) -> (Vec<String>, Vec<String>) {
        (self.undo.iter().rev().map(|s| s.name.clone()).collect(), self.redo.iter().rev().map(|s| s.name.clone()).collect())
    }

    /// Puts `state` back to before the last step. Returns whether there was one.
    pub fn undo(&mut self, state: &mut T) -> bool {
        self.commit_open(state);
        let Some(step) = self.undo.pop() else { return false };
        state.clone_from(&step.before);
        self.redo.push(step);
        true
    }

    pub fn redo(&mut self, state: &mut T) -> bool {
        self.commit_open(state);
        let Some(step) = self.redo.pop() else { return false };
        state.clone_from(&step.after);
        self.undo.push(step);
        true
    }

    /// A step still open when undo is asked for is finished first, so it is the one undone.
    fn commit_open(&mut self, state: &T) {
        if self.open.is_some() {
            self.commit(state);
        }
    }
}

/// A document whose undo keeps whole snapshots of what it edits: where its history is, and how to take and put back
/// a snapshot. Its steps then come from [`Undoable`].
pub trait Snapshots {
    type State: Clone + PartialEq;

    fn history(&self) -> &History<Self::State>;
    fn history_mut(&mut self) -> &mut History<Self::State>;
    fn snapshot(&self) -> Self::State;
    /// Puts back a snapshot that undo or redo returned to.
    fn restore(&mut self, state: Self::State);
}

/// Undo and redo in a document that keeps its own history, whatever it keeps, so the Edit menu and the toolbar can
/// work whichever editor is in front.
pub trait Undoable {
    /// Starts an undo step called `name`, unless one is open: the edits until [`Self::end_edit`] are one step.
    fn begin_edit(&mut self, name: &str);
    fn end_edit(&mut self);
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
    /// The names of the steps to undo, latest first, and to redo, next first.
    fn history_names(&self) -> (Vec<String>, Vec<String>);
    fn undo(&mut self);
    fn redo(&mut self);
}

impl<D: Snapshots> Undoable for D {
    fn begin_edit(&mut self, name: &str) {
        if !self.history().is_open() {
            let state = self.snapshot();
            self.history_mut().begin(name, &state);
        }
    }

    fn end_edit(&mut self) {
        if self.history().is_open() {
            let state = self.snapshot();
            self.history_mut().commit(&state);
        }
    }

    fn can_undo(&self) -> bool {
        self.history().can_undo()
    }

    fn can_redo(&self) -> bool {
        self.history().can_redo()
    }

    fn history_names(&self) -> (Vec<String>, Vec<String>) {
        self.history().names()
    }

    fn undo(&mut self) {
        let mut state = self.snapshot();
        if self.history_mut().undo(&mut state) {
            self.restore(state);
        }
    }

    fn redo(&mut self) {
        let mut state = self.snapshot();
        if self.history_mut().redo(&mut state) {
            self.restore(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_is_one_step_and_comes_back_on_redo() {
        let mut history = History::default();
        let mut state = vec![0; 4];
        history.begin("Brush", &state);
        state[0] = 1;
        history.begin("Brush", &state);
        state[1] = 1;
        history.commit(&state);
        assert_eq!(history.names().0, ["Brush"]);
        assert!(history.undo(&mut state));
        assert_eq!(state, [0; 4]);
        assert!(history.redo(&mut state));
        assert_eq!(state, [1, 1, 0, 0]);
        assert!(!history.redo(&mut state));
    }

    #[test]
    fn nothing_changed_is_no_step_and_a_new_edit_drops_the_redo() {
        let mut history = History::default();
        let mut state = 1;
        history.record("Nothing", 1, &state);
        assert!(!history.can_undo());
        history.record("One", 1, &2);
        state = 2;
        history.undo(&mut state);
        history.record("Other", 1, &3);
        assert!(!history.can_redo());
        assert_eq!(history.names().0, ["Other"]);
    }
}
