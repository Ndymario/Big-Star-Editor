use crate::{Error, ErrorCode, Result};

/// An undoable edit of a `T`. The history owns the commands and lends them the target on every call, so a
/// command never holds a reference into the thing it edits.
pub trait Command<T> {
    fn name(&self) -> &str;
    fn execute(&mut self, target: &mut T) -> Result<()>;
    fn undo(&mut self, target: &mut T) -> Result<()>;
}

pub struct CommandHistory<T> {
    commands: Vec<Box<dyn Command<T>>>,
    position: usize,
    /// `None` once the clean state has been undone past and overwritten.
    clean_position: Option<usize>,
}

impl<T> Default for CommandHistory<T> {
    fn default() -> Self {
        Self { commands: Vec::new(), position: 0, clean_position: Some(0) }
    }
}

impl<T> CommandHistory<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, target: &mut T, mut command: Box<dyn Command<T>>) -> Result<()> {
        command.execute(target)?;
        if self.position < self.commands.len() {
            if self.clean_position.is_some_and(|clean| clean > self.position) {
                self.clean_position = None;
            }
            self.commands.truncate(self.position);
        }
        self.commands.push(command);
        self.position += 1;
        Ok(())
    }

    pub fn undo(&mut self, target: &mut T) -> Result<()> {
        let index = self.position.checked_sub(1).ok_or_else(|| Error::new(ErrorCode::InvalidArgument, "nothing to undo"))?;
        let command = self.commands.get_mut(index).ok_or_else(|| Error::new(ErrorCode::InvalidArgument, "nothing to undo"))?;
        command.undo(target)?;
        self.position -= 1;
        Ok(())
    }

    pub fn redo(&mut self, target: &mut T) -> Result<()> {
        let command = self.commands.get_mut(self.position).ok_or_else(|| Error::new(ErrorCode::InvalidArgument, "nothing to redo"))?;
        command.execute(target)?;
        self.position += 1;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.position = 0;
        self.clean_position = Some(0);
    }

    pub fn mark_clean(&mut self) {
        self.clean_position = Some(self.position);
    }

    pub fn can_undo(&self) -> bool {
        self.position != 0
    }

    pub fn can_redo(&self) -> bool {
        self.position < self.commands.len()
    }

    pub fn dirty(&self) -> bool {
        self.clean_position != Some(self.position)
    }

    pub fn undo_name(&self) -> String {
        self.undo_names().into_iter().next().unwrap_or_default()
    }

    pub fn redo_name(&self) -> String {
        self.redo_names().into_iter().next().unwrap_or_default()
    }

    /// Most recent first.
    pub fn undo_names(&self) -> Vec<String> {
        self.commands.iter().take(self.position).rev().map(|c| c.name().to_owned()).collect()
    }

    /// Next to redo first.
    pub fn redo_names(&self) -> Vec<String> {
        self.commands.iter().skip(self.position).map(|c| c.name().to_owned()).collect()
    }
}
