//! The undoable edits an [`EditableLevel`](crate::EditableLevel) is made of. Each command is applied to a
//! [`LevelState`] the history lends it, and reaches the thing it edits through a plain function pointer.

use bigstar_core::{Command, Error, ErrorCode, Result};

use crate::state::{LevelState, LevelTilesetReferences};
use bigstar_formats::NsmbLevelScreen;

pub(crate) type Field<V> = fn(&mut LevelState) -> &mut V;
pub(crate) type Cmd = Box<dyn Command<LevelState>>;

fn out_of_bounds(message: &str) -> Error {
    Error::new(ErrorCode::OutOfBounds, message)
}

pub(crate) struct Replace<V> {
    field: Field<Vec<V>>,
    index: usize,
    before: V,
    after: V,
    name: String,
}

impl<V: Clone> Replace<V> {
    pub(crate) fn new(state: &mut LevelState, field: Field<Vec<V>>, index: usize, after: V, name: &str) -> Result<Self> {
        let before = field(state).get(index).cloned().ok_or_else(|| out_of_bounds("item index is outside the collection"))?;
        Ok(Self { field, index, before, after, name: name.to_owned() })
    }
}

impl<V: Clone + Send> Command<LevelState> for Replace<V> {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, target: &mut LevelState) -> Result<()> {
        let slot = (self.field)(target).get_mut(self.index).ok_or_else(|| out_of_bounds("item to edit no longer exists"))?;
        *slot = self.after.clone();
        Ok(())
    }

    fn undo(&mut self, target: &mut LevelState) -> Result<()> {
        let slot = (self.field)(target).get_mut(self.index).ok_or_else(|| out_of_bounds("edited item no longer exists"))?;
        *slot = self.before.clone();
        Ok(())
    }
}

pub(crate) struct Value<V> {
    field: Field<V>,
    before: V,
    after: V,
    name: String,
}

impl<V: Clone> Value<V> {
    pub(crate) fn new(state: &mut LevelState, field: Field<V>, after: V, name: &str) -> Self {
        let before = field(state).clone();
        Self { field, before, after, name: name.to_owned() }
    }
}

impl<V: Clone + Send> Command<LevelState> for Value<V> {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, target: &mut LevelState) -> Result<()> {
        *(self.field)(target) = self.after.clone();
        Ok(())
    }

    fn undo(&mut self, target: &mut LevelState) -> Result<()> {
        *(self.field)(target) = self.before.clone();
        Ok(())
    }
}

pub(crate) struct Insert<V> {
    field: Field<Vec<V>>,
    index: usize,
    value: V,
    name: String,
}

impl<V> Insert<V> {
    pub(crate) fn new(field: Field<Vec<V>>, index: usize, value: V, name: &str) -> Self {
        Self { field, index, value, name: name.to_owned() }
    }
}

impl<V: Clone + Send> Command<LevelState> for Insert<V> {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, target: &mut LevelState) -> Result<()> {
        let values = (self.field)(target);
        if self.index > values.len() {
            return Err(out_of_bounds("insert position is outside the collection"));
        }
        values.insert(self.index, self.value.clone());
        Ok(())
    }

    fn undo(&mut self, target: &mut LevelState) -> Result<()> {
        let values = (self.field)(target);
        if self.index >= values.len() {
            return Err(out_of_bounds("inserted item no longer exists"));
        }
        values.remove(self.index);
        Ok(())
    }
}

pub(crate) struct Erase<V> {
    field: Field<Vec<V>>,
    index: usize,
    value: V,
    name: String,
}

impl<V: Clone> Erase<V> {
    pub(crate) fn new(state: &mut LevelState, field: Field<Vec<V>>, index: usize, name: &str) -> Result<Self> {
        let value = field(state).get(index).cloned().ok_or_else(|| out_of_bounds("item index is outside the collection"))?;
        Ok(Self { field, index, value, name: name.to_owned() })
    }
}

impl<V: Clone + Send> Command<LevelState> for Erase<V> {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, target: &mut LevelState) -> Result<()> {
        let values = (self.field)(target);
        if self.index >= values.len() {
            return Err(out_of_bounds("item to delete no longer exists"));
        }
        values.remove(self.index);
        Ok(())
    }

    fn undo(&mut self, target: &mut LevelState) -> Result<()> {
        let values = (self.field)(target);
        if self.index > values.len() {
            return Err(out_of_bounds("delete position is outside the collection"));
        }
        values.insert(self.index, self.value.clone());
        Ok(())
    }
}

pub(crate) struct Tileset {
    before: LevelTilesetReferences,
    after: LevelTilesetReferences,
    had_background: bool,
    had_foreground: bool,
    had_tileset: bool,
}

impl Tileset {
    pub(crate) fn new(state: &LevelState, after: LevelTilesetReferences) -> Self {
        Self {
            before: state.tileset_references(),
            after,
            had_background: !state.course.backgrounds.is_empty(),
            had_foreground: !state.course.foregrounds.is_empty(),
            had_tileset: !state.course.tilesets.is_empty(),
        }
    }
}

fn set_first_screen(screens: &mut Vec<NsmbLevelScreen>, screen_id: u16, palette_id: u16) {
    match screens.first_mut() {
        Some(screen) => {
            screen.screen_id = screen_id;
            screen.palette_id = palette_id;
        }
        None => screens.push(NsmbLevelScreen { screen_id, palette_id, ..NsmbLevelScreen::default() }),
    }
}

impl Tileset {
    fn apply(target: &mut LevelState, value: LevelTilesetReferences) {
        target.course.header.tileset_theme[0] = value.tileset_id;
        // The tileset block names no screen; only its palette is the level's to choose.
        match target.course.tilesets.first_mut() {
            Some(block) => block.palette_id = value.tileset_palette_id,
            None => target.course.tilesets.push(NsmbLevelScreen { palette_id: value.tileset_palette_id, ..NsmbLevelScreen::default() }),
        }
        target.course.header.background_theme[0] = value.background_graphics_id;
        set_first_screen(&mut target.course.backgrounds, value.background_screen_id, value.background_palette_id);
        target.course.header.foreground_theme[0] = value.foreground_graphics_id;
        set_first_screen(&mut target.course.foregrounds, value.foreground_screen_id, value.foreground_palette_id);
    }
}

impl Command<LevelState> for Tileset {
    fn name(&self) -> &str {
        "Edit tileset references"
    }

    fn execute(&mut self, target: &mut LevelState) -> Result<()> {
        Self::apply(target, self.after);
        Ok(())
    }

    fn undo(&mut self, target: &mut LevelState) -> Result<()> {
        Self::apply(target, self.before);
        if !self.had_background {
            target.course.backgrounds.clear();
        }
        if !self.had_foreground {
            target.course.foregrounds.clear();
        }
        if !self.had_tileset {
            target.course.tilesets.clear();
        }
        Ok(())
    }
}

pub(crate) struct Composite {
    commands: Vec<Cmd>,
    name: String,
}

impl Composite {
    pub(crate) fn new(commands: Vec<Cmd>, name: &str) -> Self {
        Self { commands, name: name.to_owned() }
    }
}

impl Command<LevelState> for Composite {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, target: &mut LevelState) -> Result<()> {
        for completed in 0..self.commands.len() {
            let result = self.commands.get_mut(completed).map_or(Ok(()), |command| command.execute(target));
            if let Err(error) = result {
                for done in self.commands.iter_mut().take(completed).rev() {
                    let _ = done.undo(target);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn undo(&mut self, target: &mut LevelState) -> Result<()> {
        for command in self.commands.iter_mut().rev() {
            command.undo(target)?;
        }
        Ok(())
    }
}
