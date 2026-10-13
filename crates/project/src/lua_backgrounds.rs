//! Background scripts: what each of a level's two scrolling layers is called and how the game animates it. The
//! types they fill are `bigstar_level`'s; the contract is `docs/lua-backgrounds.md`.

use std::path::{Path, PathBuf};

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_level::{
    BACKGROUND_FX_ONE, BackgroundCatalog, BackgroundDefinition, BackgroundLayer, BackgroundMotion, BackgroundPaletteAnimation, BackgroundStrip,
    BackgroundSwap, BackgroundTileAnimation, BackgroundTileCopy, BackgroundTileStep,
};
use mlua::{ChunkMode, Table, Value};

use crate::lua_sandbox::{MAX_SCRIPT_BYTES, integer_field, lua_message, new_state, string_field};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LuaBackgroundLayer {
    pub name: String,
    pub directory: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LuaBackgroundCatalog {
    pub catalog: BackgroundCatalog,
    /// Every script read, for the watcher.
    pub sources: Vec<PathBuf>,
    /// Scripts that would not load, each naming itself. The rest of the catalog stands.
    pub errors: Vec<String>,
}

/// A band the script gave no channel moves on a counter of its own, numbered from here so it never meets one the
/// script chose.
const PRIVATE_CHANNEL: usize = 128;
const MAX_ENTRIES: usize = 4096;

fn parse_error(message: String) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

/// A Lua number of either kind, as the game's fixed point.
fn fixed_field(table: &Table, key: &str, what: &str) -> Result<Option<i32>> {
    let value = match table.get::<Value>(key) {
        Ok(Value::Nil) => return Ok(None),
        Ok(Value::Integer(value)) => value as f64,
        Ok(Value::Number(value)) if value.is_finite() => value,
        _ => return Err(parse_error(format!("{what} has an invalid {key}"))),
    };
    let fixed = (value * f64::from(BACKGROUND_FX_ONE)).round();
    if fixed.abs() > f64::from(i32::MAX) {
        return Err(parse_error(format!("{what} has an invalid {key}")));
    }
    Ok(Some(fixed as i32))
}

fn layer_field(table: &Table, what: &str, fallback: Option<BackgroundLayer>) -> Result<BackgroundLayer> {
    match table.get::<Value>("layer") {
        Ok(Value::Nil) => fallback.ok_or_else(|| parse_error(format!("{what} needs a layer, \"background\" or \"foreground\""))),
        Ok(Value::String(text)) => match text.to_string_lossy().as_str() {
            "background" => Ok(BackgroundLayer::Background),
            "foreground" => Ok(BackgroundLayer::Foreground),
            _ => Err(parse_error(format!("{what} has a layer that is neither \"background\" nor \"foreground\""))),
        },
        _ => Err(parse_error(format!("{what} has an invalid layer"))),
    }
}

/// The entries of a list field, in order.
fn list(table: &Table, key: &str, what: &str, required: bool) -> Result<Vec<Value>> {
    match table.get::<Value>(key) {
        Ok(Value::Nil) if !required => Ok(Vec::new()),
        Ok(Value::Table(entries)) => {
            let length = entries.raw_len();
            if length > MAX_ENTRIES {
                return Err(parse_error(format!("{what} has too many {key}")));
            }
            (1..=length).map(|index| entries.raw_get::<Value>(index).map_err(|e| parse_error(lua_message(&e)))).collect()
        }
        _ => Err(parse_error(format!("{what} needs a {key} list"))),
    }
}

fn read_strips(table: &Table, what: &str) -> Result<Vec<BackgroundStrip>> {
    let entries = list(table, "strips", what, false)?;
    let mut strips = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let what = format!("{what} strip {}", index + 1);
        let Value::Table(entry) = entry else { return Err(parse_error(format!("{what} must be a table"))) };
        let last = index + 1 == entries.len();
        let through_row = integer_field(entry, "throughRow", 0, 65535, &what, !last, 65535)? as u16;
        let scroll = fixed_field(entry, "scroll", &what)?;
        let drift = fixed_field(entry, "drift", &what)?;
        let wave = match entry.get::<Value>("wave") {
            Ok(Value::Nil) => None,
            Ok(Value::Table(wave)) => Some(BackgroundMotion::Wave {
                amplitude: fixed_field(&wave, "amplitude", &what)?.ok_or_else(|| parse_error(format!("{what} wave needs an amplitude")))?,
                speed: integer_field(&wave, "speed", 0, 65535, &what, true, 0)? as u16,
                step: integer_field(&wave, "step", 0, 65535, &what, true, 0)? as u16,
            }),
            _ => return Err(parse_error(format!("{what} has an invalid wave"))),
        };
        let motion = match (scroll, drift, wave) {
            (Some(factor), None, None) => BackgroundMotion::Scroll { factor },
            (None, Some(speed), None) => {
                let channel = integer_field(entry, "channel", 0, PRIVATE_CHANNEL as i64 - 1, &what, false, (PRIVATE_CHANNEL + index) as i64)?;
                BackgroundMotion::Drift { speed, channel: channel.min(255) as u8 }
            }
            (None, None, Some(wave)) => wave,
            _ => return Err(parse_error(format!("{what} needs exactly one of scroll, drift and wave"))),
        };
        strips.push(BackgroundStrip { through_row, motion });
    }
    Ok(strips)
}

fn read_tiles(table: &Table, what: &str, own: BackgroundLayer) -> Result<Option<BackgroundTileAnimation>> {
    let tiles = match table.get::<Value>("tiles") {
        Ok(Value::Nil) => return Ok(None),
        Ok(Value::Table(tiles)) => tiles,
        _ => return Err(parse_error(format!("{what} has an invalid tiles"))),
    };
    let what = format!("{what} tiles");
    let mut animation = BackgroundTileAnimation { file: string_field(&tiles, "file", &what, true)?, ..Default::default() };
    let interval = integer_field(&tiles, "interval", 1, 65535, &what, false, 0)?;
    for (index, entry) in list(&tiles, "frames", &what, true)?.iter().enumerate() {
        let what = format!("{what} frame {}", index + 1);
        let (tile, ticks) = match entry {
            Value::Integer(tile) => (*tile, interval),
            // `{ tile, ticks = n }`: a frame that lasts its own time.
            Value::Table(frame) => {
                let Ok(Value::Integer(tile)) = frame.raw_get::<Value>(1) else { return Err(parse_error(format!("{what} needs a tile"))) };
                (tile, integer_field(frame, "ticks", 1, 65535, &what, false, interval)?)
            }
            _ => return Err(parse_error(format!("{what} must be a tile or a table"))),
        };
        if !(0..=65535).contains(&tile) {
            return Err(parse_error(format!("{what} has an invalid tile")));
        }
        if ticks == 0 {
            return Err(parse_error(format!("{what} has no ticks, and the animation no interval")));
        }
        animation.steps.push(BackgroundTileStep { tile: tile as u16, ticks: ticks as u16 });
    }
    for (index, entry) in list(&tiles, "copies", &what, true)?.iter().enumerate() {
        let what = format!("{what} copy {}", index + 1);
        let Value::Table(copy) = entry else { return Err(parse_error(format!("{what} must be a table"))) };
        animation.copies.push(BackgroundTileCopy {
            layer: layer_field(copy, &what, Some(own))?,
            to: integer_field(copy, "to", 0, 65535, &what, true, 0)? as u16,
            count: integer_field(copy, "count", 1, 65535, &what, true, 0)? as u16,
            from: integer_field(copy, "from", 0, 65535, &what, false, 0)? as u16,
        });
    }
    if animation.steps.is_empty() || animation.copies.is_empty() {
        return Err(parse_error(format!("{what} needs at least one frame and one copy")));
    }
    Ok(Some(animation))
}

fn read_palette(table: &Table, what: &str) -> Result<Option<BackgroundPaletteAnimation>> {
    let palette = match table.get::<Value>("palette") {
        Ok(Value::Nil) => return Ok(None),
        Ok(Value::Table(palette)) => palette,
        _ => return Err(parse_error(format!("{what} has an invalid palette"))),
    };
    let what = format!("{what} palette");
    let count = integer_field(&palette, "count", 1, 65535, &what, true, 0)?;
    Ok(Some(BackgroundPaletteAnimation {
        file: string_field(&palette, "file", &what, true)?,
        ticks: integer_field(&palette, "interval", 1, 65535, &what, true, 0)? as u16,
        frames: integer_field(&palette, "frames", 1, 65535, &what, true, 0)? as u16,
        to: integer_field(&palette, "to", 0, 65535, &what, true, 0)? as u16,
        count: count as u16,
        stride: integer_field(&palette, "stride", 1, 65535, &what, false, count)? as u16,
    }))
}

/// A quarter of the image by name, as `(column, row)`.
fn quarter_field(table: &Table, key: &str, what: &str) -> Result<(u8, u8)> {
    match string_field(table, key, what, true)?.as_str() {
        "top-left" => Ok((0, 0)),
        "top-right" => Ok((1, 0)),
        "bottom-left" => Ok((0, 1)),
        "bottom-right" => Ok((1, 1)),
        _ => Err(parse_error(format!("{what} has a {key} that is not top-left, top-right, bottom-left or bottom-right"))),
    }
}

fn read_swaps(table: &Table, what: &str) -> Result<Vec<BackgroundSwap>> {
    let mut swaps = Vec::new();
    for (index, entry) in list(table, "swaps", what, false)?.iter().enumerate() {
        let what = format!("{what} swap {}", index + 1);
        let Value::Table(entry) = entry else { return Err(parse_error(format!("{what} must be a table"))) };
        swaps.push(BackgroundSwap {
            after: fixed_field(entry, "after", &what)?.ok_or_else(|| parse_error(format!("{what} needs an after")))?,
            quarter: quarter_field(entry, "quarter", &what)?,
            shows: quarter_field(entry, "shows", &what)?,
        });
    }
    Ok(swaps)
}

fn load_script(path: &Path) -> Result<BackgroundDefinition> {
    let bytes = read_file(path)?;
    if bytes.len() > MAX_SCRIPT_BYTES {
        return Err(Error::new(ErrorCode::OutOfBounds, "background script is larger than 1 MiB"));
    }
    // A background script is data, as a tile script is: the state dies with this call.
    let lua = new_state()?;
    let returned: Value = lua
        .load(bytes.as_slice())
        .set_name(format!("@{}", path.display()))
        .set_mode(ChunkMode::Text)
        .call(())
        .map_err(|e| parse_error(lua_message(&e)))?;
    let Value::Table(table) = returned else { return Err(parse_error("background script must return a table".to_owned())) };

    let what = "background script";
    let layer = layer_field(&table, what, None)?;
    let id = integer_field(&table, "id", 0, 255, what, true, 0)? as u16;
    let what = format!("{} {id}", layer.name());
    Ok(BackgroundDefinition {
        layer,
        id,
        name: string_field(&table, "name", &what, true)?,
        strips: read_strips(&table, &what)?,
        tiles: read_tiles(&table, &what, layer)?,
        palette: read_palette(&table, &what)?,
        swaps: read_swaps(&table, &what)?,
    })
}

/// Reads every background script of every layer. The first definition of a layer and id wins, so the project's own
/// comes before a module's and a module's before the shipped one. A script that will not load is named in `errors` and
/// costs nothing else.
pub fn load_lua_background_layers(layers: &[LuaBackgroundLayer]) -> Result<LuaBackgroundCatalog> {
    let mut result = LuaBackgroundCatalog::default();
    for layer in layers {
        match layer.directory.try_exists() {
            Err(_) => return Err(Error::new(ErrorCode::IoError, format!("could not inspect background layer {}", layer.name))),
            Ok(false) => continue,
            Ok(true) => {}
        }
        if !layer.directory.is_dir() {
            return Err(Error::new(ErrorCode::InvalidArgument, format!("background layer is not a directory: {}", layer.name)));
        }
        let entries = std::fs::read_dir(&layer.directory)
            .map_err(|_| Error::new(ErrorCode::IoError, format!("could not read background layer {}", layer.name)))?;
        let mut scripts: Vec<PathBuf> =
            entries.flatten().map(|entry| entry.path()).filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "lua")).collect();
        scripts.sort();
        for script in scripts {
            match load_script(&script) {
                Ok(definition) => {
                    if result.catalog.definitions.iter().all(|known| known.layer != definition.layer || known.id != definition.id) {
                        result.catalog.definitions.push(definition);
                    }
                }
                Err(error) => {
                    let filename = script.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                    result.errors.push(format!("{} / {filename}: {}", layer.name, error.message));
                }
            }
            result.sources.push(script);
        }
    }
    Ok(result)
}
