//! Tile scripts: which Map16 blocks the game hands to the 3D engine and what it draws in their place. The loader half
//! of `libs/nsmb-project/src/lua_tiles.cpp`; the overlay types and the substitution are in `tile_overlays`. The
//! contract is `docs/lua-tiles.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_formats::{RgbaImage, Tex0, decompress_nitro_lz, parse_tex0, render_tex0_texture};
use mlua::{ChunkMode, Table, Value};
use serde::{Deserialize, Serialize};

use crate::lua_actors::{ActorResourceKind, ActorResourceResolver};
use crate::lua_sandbox::{MAX_SCRIPT_BYTES, boolean_field, integer_field, lua_message, new_state, string_field};
use crate::tile_overlays::{TileOverlay, TileOverlayCatalog, TileOverlayScript};

/// Map16 block numbers are absolute across the three tilesets a level loads: jyotyu 0-255, the level tileset
/// 256-1023, nohara 1024-1535.
const MAX_BLOCK: i64 = 1535;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuaTileLayer {
    pub name: String,
    pub directory: PathBuf,
}

/// Keyed on the resolved resource plus everything that changes the pixels, so the three pipe colours -- the same
/// texture at three palette offsets -- decode once each and the whole of `I_dokan.nsbtx` decodes once.
#[derive(Default)]
struct TextureCache {
    images: HashMap<(String, String, usize, usize), Arc<RgbaImage>>,
    containers: HashMap<String, Arc<Tex0>>,
}

/// What one `blocks` entry asks for, before the texture is resolved.
#[derive(Default)]
struct BlockRequest {
    file: String,
    texture: String,
    texture_index: usize,
    texture_by_name: bool,
    palette: usize,
    palette_offset: usize,
    remap: Option<u16>,
    /// False when the entry is empty: claimed, drawn as nothing.
    draws: bool,
}

fn resolve_texture(
    resolve: ActorResourceResolver<'_>,
    request: &BlockRequest,
    cache: &mut TextureCache,
    watched: &mut Vec<PathBuf>,
) -> Result<Arc<RgbaImage>> {
    let found = resolve(ActorResourceKind::Model, &request.file)?;
    // The address when the ROM supplies this, the disk path when a module does.
    let resource_key = if found.source.as_os_str().is_empty() { found.rom_path.clone() } else { found.source.to_string_lossy().into_owned() };
    if !found.source.as_os_str().is_empty() {
        watched.push(found.source.clone());
    }
    let tex0 = match cache.containers.get(&resource_key) {
        Some(container) => container.clone(),
        None => {
            let bytes = match found.data.first() {
                Some(0x10 | 0x11) => decompress_nitro_lz(&found.data)?,
                _ => found.data,
            };
            let parsed = Arc::new(parse_tex0(&bytes)?);
            cache.containers.insert(resource_key.clone(), parsed.clone());
            parsed
        }
    };

    let index = if request.texture_by_name {
        tex0.textures
            .iter()
            .position(|texture| texture.name == request.texture)
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("{} has no texture named {}", request.file, request.texture)))?
    } else {
        request.texture_index
    };
    let (Some(texture), Some(palette)) = (tex0.textures.get(index), tex0.palettes.get(request.palette)) else {
        return Err(Error::new(ErrorCode::OutOfBounds, format!("{} has no such texture or palette", request.file)));
    };

    let key = (resource_key, texture.name.clone(), request.palette, request.palette_offset);
    if let Some(cached) = cache.images.get(&key) {
        return Ok(cached.clone());
    }
    // A palette offset selects a band inside one palette rather than another palette: the green, red and yellow
    // pipes are the same texels read 0, 256 and 512 colours into `I_dokan`'s single 768-colour palette.
    let skip = request.palette_offset * 2;
    if skip >= palette.colors.len() {
        return Err(Error::new(ErrorCode::OutOfBounds, format!("{} palette offset is past the end of the palette", request.file)));
    }
    let mut banded = palette.clone();
    banded.colors.drain(..skip);
    let image = Arc::new(render_tex0_texture(texture, &banded)?);
    cache.images.insert(key, image.clone());
    Ok(image)
}

fn read_block_request(entry: &Table, what: &str) -> Result<BlockRequest> {
    let mut request = BlockRequest::default();
    let remap = integer_field(entry, "remap", 0, MAX_BLOCK, what, false, -1)?;
    if remap >= 0 {
        request.remap = Some(remap as u16);
        return Ok(request);
    }
    // An empty entry claims the block and draws nothing, which is what the game does for the thin fences the boss
    // tilesets blank outright.
    let file = entry.raw_get::<Value>(1).unwrap_or(Value::Nil);
    if file.is_nil() {
        return Ok(request);
    }
    let Value::String(file) = file else { return Err(Error::new(ErrorCode::ParseError, format!("{what} needs a file name"))) };
    request.file = file.to_string_lossy();
    match entry.raw_get::<Value>(2).unwrap_or(Value::Nil) {
        Value::String(texture) => {
            request.texture = texture.to_string_lossy();
            request.texture_by_name = true;
        }
        Value::Integer(index) if (0..=65535).contains(&index) => request.texture_index = index as usize,
        Value::Nil => {}
        _ => return Err(Error::new(ErrorCode::ParseError, format!("{what} has an invalid texture"))),
    }
    request.palette = integer_field(entry, "paletteIndex", 0, 65535, what, false, 0)? as usize;
    request.palette_offset = integer_field(entry, "palette", 0, 65535, what, false, 0)? as usize;
    request.draws = true;
    Ok(request)
}

fn load_script(path: &Path, resolve: ActorResourceResolver<'_>, cache: &mut TextureCache) -> Result<TileOverlayScript> {
    let bytes = read_file(path)?;
    if bytes.len() > MAX_SCRIPT_BYTES {
        return Err(Error::new(ErrorCode::OutOfBounds, "tile script is larger than 1 MiB"));
    }
    // A tile script is data, so the state dies with this call and nothing has to be kept alive for painting.
    let lua = new_state()?;
    let returned: Value = lua
        .load(bytes.as_slice())
        .set_name(format!("@{}", path.display()))
        .set_mode(ChunkMode::Text)
        .call(())
        .map_err(|e| Error::new(ErrorCode::ParseError, lua_message(&e)))?;
    let Value::Table(table) = returned else { return Err(Error::new(ErrorCode::ParseError, "tile script must return a table")) };

    let filename = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let mut script = TileOverlayScript { source: path.to_path_buf(), name: string_field(&table, "name", &filename, true)?, ..Default::default() };
    let replaces = boolean_field(&table, "replaces", &script.name, true)?;

    match table.get::<Value>("tilesets") {
        Ok(Value::Nil) => {}
        Ok(Value::Table(tilesets)) => {
            for i in 1..=tilesets.raw_len() {
                match tilesets.raw_get::<Value>(i) {
                    Ok(Value::Integer(id)) if (0..=65535).contains(&id) => script.tilesets.push(id as u16),
                    _ => return Err(Error::new(ErrorCode::ParseError, format!("{} has an invalid tilesets entry", script.name))),
                }
            }
            // An empty list would otherwise read as "every tileset", which is the opposite of what writing one means.
            if script.tilesets.is_empty() {
                return Err(Error::new(ErrorCode::ParseError, format!("{} lists no tilesets; omit the field to mean every tileset", script.name)));
            }
        }
        _ => return Err(Error::new(ErrorCode::ParseError, format!("{} has an invalid tilesets", script.name))),
    }

    let Ok(Value::Table(blocks)) = table.get::<Value>("blocks") else {
        return Err(Error::new(ErrorCode::ParseError, format!("{} needs a blocks table", script.name)));
    };
    for pair in blocks.pairs::<Value, Value>() {
        let (key, entry) = pair.map_err(|e| Error::new(ErrorCode::ParseError, lua_message(&e)))?;
        let block = match key {
            Value::Integer(block) if (0..=MAX_BLOCK).contains(&block) => block as u16,
            _ => return Err(Error::new(ErrorCode::ParseError, format!("{} has a block number outside 0-1535", script.name))),
        };
        let Value::Table(entry) = entry else {
            return Err(Error::new(ErrorCode::ParseError, format!("{} block {block} must be a table", script.name)));
        };
        let what = format!("{} block {block}", script.name);
        let request = read_block_request(&entry, &what)?;
        let mut overlay = TileOverlay { replaces, remap: request.remap, image: None };
        if request.draws {
            // A texture the ROM does not hold costs that block and nothing else, so a module missing one asset still
            // contributes the rest.
            match resolve_texture(resolve, &request, cache, &mut script.resources) {
                Ok(image) => overlay.image = Some(image),
                Err(error) => script.errors.push(format!("{what}: {}", error.message)),
            }
        }
        script.blocks.entry(block).or_insert(overlay);
    }
    script.resources.sort();
    script.resources.dedup();
    Ok(script)
}

/// Tile scripts are data: they name a block, an NSBTX and a texture, and this resolves the texture through the same
/// resolver the actor loader uses, so a module's replacement NSBTX is picked up the same way an actor's is.
pub fn load_lua_tile_layers(layers: &[LuaTileLayer], resolve: ActorResourceResolver<'_>) -> Result<TileOverlayCatalog> {
    let mut result = TileOverlayCatalog::default();
    let mut cache = TextureCache::default();
    for layer in layers {
        match layer.directory.try_exists() {
            Err(_) => return Err(Error::new(ErrorCode::IoError, format!("could not inspect tile layer {}", layer.name))),
            Ok(false) => continue,
            Ok(true) => {}
        }
        if !layer.directory.is_dir() {
            return Err(Error::new(ErrorCode::InvalidArgument, format!("tile layer is not a directory: {}", layer.name)));
        }
        let entries =
            std::fs::read_dir(&layer.directory).map_err(|_| Error::new(ErrorCode::IoError, format!("could not read tile layer {}", layer.name)))?;
        let mut scripts: Vec<PathBuf> =
            entries.flatten().map(|entry| entry.path()).filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "lua")).collect();
        scripts.sort();
        for script in scripts {
            let loaded = load_script(&script, resolve, &mut cache).map_err(|e| {
                let filename = script.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                Error::new(e.code, format!("{} / {filename}: {}", layer.name, e.message))
            })?;
            result.scripts.push(loaded);
        }
    }
    Ok(result)
}
