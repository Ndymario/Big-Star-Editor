//! The parts of a sandboxed Lua state that have nothing to do with what the script is for: the bounded state, the
//! safe standard library, and the typed table readers every loader needs to report a bad field by name. Port of
//! `libs/nsmb-project/src/lua_sandbox.cpp` on `mlua` (Lua 5.4, vendored).
//!
//! Two loaders share this -- actors, which run code on every paint, and tiles, which are pure data -- so it holds no
//! notion of a renderer, a resource context or a draw command.

use bigstar_core::{Error, ErrorCode, Result};
use mlua::{Lua, LuaOptions, StdLib, Table, Value};

pub(crate) const MAX_SCRIPT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_LUA_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_INSTRUCTIONS: usize = 200_000;

/// A state with the pure parts of the standard library and a memory cap. Scripts format numbers into labels and
/// resource names, so `string`, `math`, `table` and the base library are available; anything that reaches the
/// filesystem, the process, or the loader stays out.
pub(crate) fn new_state() -> Result<Lua> {
    let failed = |e: mlua::Error| Error::new(ErrorCode::IoError, format!("could not create the Lua state: {e}"));
    let lua = Lua::new_with(StdLib::STRING | StdLib::MATH | StdLib::TABLE, LuaOptions::new()).map_err(failed)?;
    lua.set_memory_limit(MAX_LUA_BYTES).map_err(failed)?;
    let globals = lua.globals();
    for name in ["dofile", "loadfile", "load", "require", "collectgarbage", "print", "loadstring"] {
        globals.raw_set(name, Value::Nil).map_err(failed)?;
    }
    Ok(lua)
}

/// What Lua said, without mlua's wrapping: the message a C `lua_tostring` on the error object would have given.
pub(crate) fn lua_message(error: &mlua::Error) -> String {
    let message = match error {
        mlua::Error::CallbackError { cause, .. } => return lua_message(cause),
        mlua::Error::RuntimeError(message) | mlua::Error::MemoryError(message) => message.clone(),
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        mlua::Error::BadArgument { cause, .. } => return lua_message(cause),
        other => other.to_string(),
    };
    match message.find("\nstack traceback:") {
        Some(end) => message.get(..end).unwrap_or_default().to_owned(),
        None => message,
    }
}

fn invalid(what: &str, key: &str) -> Error {
    Error::new(ErrorCode::ParseError, format!("{what} has an invalid {key}"))
}

/// `what` names the thing being read in the error message -- an actor's name, a tile script's filename -- so a bad
/// field says which file to open. Only an integer-typed Lua value counts, as `lua_isinteger` decides.
pub(crate) fn integer_field(table: &Table, key: &str, minimum: i64, maximum: i64, what: &str, required: bool, fallback: i64) -> Result<i64> {
    match table.get::<Value>(key) {
        Ok(Value::Nil) if !required => Ok(fallback),
        Ok(Value::Integer(value)) if (minimum..=maximum).contains(&value) => Ok(value),
        _ => Err(invalid(what, key)),
    }
}

pub(crate) fn string_field(table: &Table, key: &str, what: &str, required: bool) -> Result<String> {
    match table.get::<Value>(key) {
        Ok(Value::Nil) if !required => Ok(String::new()),
        Ok(Value::String(text)) => {
            let text = text.to_string_lossy();
            if required && text.is_empty() {
                return Err(Error::new(ErrorCode::ParseError, format!("{what} has an empty {key}")));
            }
            Ok(text)
        }
        _ => Err(invalid(what, key)),
    }
}

pub(crate) fn boolean_field(table: &Table, key: &str, what: &str, fallback: bool) -> Result<bool> {
    match table.get::<Value>(key) {
        Ok(Value::Nil) => Ok(fallback),
        Ok(Value::Boolean(value)) => Ok(value),
        _ => Err(invalid(what, key)),
    }
}
