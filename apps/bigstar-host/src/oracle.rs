//! `oracle <project> <data-dir>`: one line per fact about a project, printed exactly as `nsmb-cli oracle` prints it,
//! so `cargo xtask oracle` can diff the C++ and Rust implementations on a real ROM. Every ROM file's format and the
//! hash of what parsing and writing it back produces, every course's entities and decoded graphics, and every shipped
//! actor's bounds and draw list at default settings. Error messages differ between the two by design, so only error
//! codes are printed.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_formats::{
    RgbaImage, UiResourceKind, compress_blz_overlay, compress_nitro_lz10, compress_nitro_lz11, decompress_nitro_lz, format_name, identify_format,
    identify_ui_resource, is_bmg, is_narc, parse_bmg, parse_bnbl, parse_bncd, parse_bncl, parse_ds_banner, parse_enpg, parse_nanr, parse_narc,
    parse_ncer, parse_ncgr, parse_nclr, parse_nscr, parse_tex0, render_tex0_texture, write_bmg, write_bnbl, write_bncd, write_bncl, write_ds_banner,
    write_enpg, write_nanr, write_narc, write_ncer, write_ncgr, write_nclr, write_nscr, write_tex0,
};
use bigstar_level::{EditableLevel, SpriteBounds, rasterize_level_objects};
use bigstar_project::{
    ActorResource, ActorResourceKind, EffectiveFilesystem, LuaActorDrawKind, LuaActorLayer, LuaActorRenderInput, LuaTileLayer, MANIFEST_FILE_NAME,
    RomReader, TileOverlayCatalog, has_lua_renderer, load_background_graphics, load_level_tilesets, load_lua_actor_layers, load_lua_tile_layers,
    load_manifest, lua_actor_bounds, render_lua_actor, resolve_level_graphics,
};
use sha2::{Digest, Sha256};

fn hex(hash: Sha256) -> String {
    hash.finalize().iter().fold(String::with_capacity(64), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn sha(data: &[u8]) -> String {
    hex(Sha256::new_with_prefix(data))
}

fn sha_image(image: &RgbaImage) -> String {
    let mut hash = Sha256::new();
    hash.update((image.width as u32).to_le_bytes());
    hash.update((image.height as u32).to_le_bytes());
    hash.update(&image.pixels);
    hex(hash)
}

fn failed(error: &Error) -> String {
    format!("E:{:?}", error.code)
}

fn hashed(result: &Result<Vec<u8>>) -> String {
    match result {
        Ok(data) => sha(data),
        Err(error) => failed(error),
    }
}

fn number(value: f64) -> String {
    format!("{value:.4}")
}

/// Parse, then write back, then hash -- or the error code of whichever step failed.
fn round_trip<T>(parsed: Result<T>, write: impl Fn(&T) -> Result<Vec<u8>>) -> String {
    match parsed {
        Ok(value) => hashed(&write(&value)),
        Err(error) => failed(&error),
    }
}

/// What parsing and writing back produces, for the formats that have both.
fn rewrite(data: &[u8], path: &str) -> String {
    if data.starts_with(b"RLCN") {
        return format!(" nclr {}", round_trip(parse_nclr(data), write_nclr));
    }
    if data.starts_with(b"RGCN") {
        return format!(" ncgr {}", round_trip(parse_ncgr(data), write_ncgr));
    }
    if data.starts_with(b"RCSN") {
        return format!(" nscr {}", round_trip(parse_nscr(data), write_nscr));
    }
    if data.starts_with(b"RECN") {
        return format!(" ncer {}", round_trip(parse_ncer(data), write_ncer));
    }
    if data.starts_with(b"RNAN") {
        return format!(" nanr {}", round_trip(parse_nanr(data), write_nanr));
    }
    if data.starts_with(b"BTX0") || data.starts_with(b"BMD0") || data.starts_with(b"TEX0") {
        let tex0 = match parse_tex0(data) {
            Ok(tex0) => tex0,
            Err(error) => return format!(" tex0 {}", failed(&error)),
        };
        let mut rendered = Sha256::new();
        let palette = tex0.palettes.first().cloned().unwrap_or_default();
        for texture in &tex0.textures {
            rendered.update(match render_tex0_texture(texture, &palette) {
                Ok(image) => sha_image(&image),
                Err(error) => failed(&error),
            });
        }
        return format!(" tex0 {} textures={} render={}", hashed(&write_tex0(&tex0)), tex0.textures.len(), hex(rendered));
    }
    if is_bmg(data, path) {
        return match parse_bmg(data) {
            Ok(bmg) => format!(" bmg {} messages={}", hashed(&write_bmg(&bmg)), bmg.messages.len()),
            Err(error) => format!(" bmg {}", failed(&error)),
        };
    }
    match identify_ui_resource(data, path) {
        UiResourceKind::Bnbl => format!(" bnbl {}", round_trip(parse_bnbl(data), write_bnbl)),
        UiResourceKind::Bncl => format!(" bncl {}", round_trip(parse_bncl(data), write_bncl)),
        UiResourceKind::Bncd => format!(" bncd {}", round_trip(parse_bncd(data), write_bncd)),
        UiResourceKind::Enpg => format!(" enpg {}", round_trip(parse_enpg(data), write_enpg)),
        _ => String::new(),
    }
}

fn report_file(out: &mut dyn Write, indent: &str, label: &str, path: &str, data: &[u8], members: bool) {
    let mut line = format!("{indent}{label} {} size={}", format_name(identify_format(data, path)), data.len());
    let mut raw = data.to_vec();
    if let Some(&variant @ (0x10 | 0x11)) = data.first()
        && let Ok(unpacked) = decompress_nitro_lz(data)
    {
        let repacked = if variant == 0x10 { compress_nitro_lz10(&unpacked) } else { compress_nitro_lz11(&unpacked) };
        let _ = write!(line, " {} raw={} repack={}", if variant == 0x10 { "lz10" } else { "lz11" }, sha(&unpacked), hashed(&repacked));
        raw = unpacked;
    }
    if is_narc(&raw) {
        let narc = match parse_narc(&raw) {
            Ok(narc) => narc,
            Err(error) => {
                let _ = writeln!(out, "{line} narc {}", failed(&error));
                return;
            }
        };
        let _ = writeln!(out, "{line} narc {} members={}", hashed(&write_narc(&narc)), narc.files.len());
        if members {
            for (i, member) in narc.files.iter().enumerate() {
                report_file(out, &format!("{indent}  "), &format!("member {i}"), &format!("{path}!{i}"), member, false);
            }
        }
        return;
    }
    let _ = writeln!(out, "{line}{}", rewrite(&raw, path));
}

fn report_level(out: &mut dyn Write, files: &RefCell<EffectiveFilesystem>, course_path: &str, bgdat_path: &str, overlays: &TileOverlayCatalog) {
    let _ = writeln!(out, "level {course_path}");
    let course = files.borrow_mut().read(course_path);
    let bgdat = files.borrow_mut().read(bgdat_path);
    let (course, bgdat) = match (course, bgdat) {
        (Ok(course), Ok(bgdat)) => (course, bgdat),
        (Err(error), _) | (_, Err(error)) => {
            let _ = writeln!(out, "  read {}", failed(&error));
            return;
        }
    };
    let level = match EditableLevel::load(&course, &bgdat, None) {
        Ok(level) => level,
        Err(error) => {
            let _ = writeln!(out, "  load {}", failed(&error));
            return;
        }
    };
    let c = level.course();
    let h = &c.header;
    let _ = write!(out, "  header {} {} {} {}", h.entrances[0], h.entrances[1], h.settings, h.timer);
    for themes in [h.background_theme, h.tileset_theme, h.foreground_theme] {
        for theme in themes {
            let _ = write!(out, " {theme}");
        }
    }
    let _ = writeln!(out, " {} {} {} {}", h.events, h.sound_group, h.sprite_priority, h.mega_drop_chance);
    for camera in &c.cameras {
        let _ = writeln!(
            out,
            "  camera {} {} {} {} {} {} {}",
            camera.top_offset,
            camera.bottom_offset,
            camera.top_offset_special,
            camera.bottom_offset_special,
            camera.id,
            camera.scroll_height,
            camera.unknown
        );
    }
    for (name, list) in [("background", &c.backgrounds), ("tileset", &c.tilesets), ("foreground", &c.foregrounds)] {
        for s in list {
            let _ = writeln!(
                out,
                "  {name} {} {} {} {} {} {} {} {} {} {}",
                s.id, s.screen_id, s.palette_id, s.parallax_x, s.parallax_y, s.scroll_x, s.scroll_y, s.vertical_offset, s.unused_10, s.unused_12
            );
        }
    }
    for o in level.objects() {
        let _ = writeln!(out, "  object {} {} {} {} {} {}", o.object_id, o.tileset_slot, o.x, o.y, o.width, o.height);
    }
    for s in &c.sprites {
        let _ = writeln!(out, "  sprite {} {} {} {} {} {}", s.id, s.x, s.y, s.events[0], s.events[1], s.settings);
    }
    for e in &c.entrances {
        let _ = writeln!(
            out,
            "  entrance {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            e.x,
            e.y,
            e.camera_x,
            e.camera_y,
            e.id,
            e.target_stage_area,
            e.target_stage_index,
            e.target_stage_group,
            e.target_entrance,
            e.camera_zoom,
            e.kind,
            e.flags,
            e.unknown_10,
            e.unknown_11,
            e.view_id,
            e.target_worldmap_node
        );
    }
    for v in &c.views {
        let _ = writeln!(
            out,
            "  view {} {} {} {} {} {} {} {} {} {} {} {}",
            v.x,
            v.y,
            v.width,
            v.height,
            v.id,
            v.camera_settings_id,
            v.bgm_id,
            v.background_id,
            v.tileset_id,
            v.foreground_id,
            v.light_type,
            v.progress_path_id
        );
    }
    for z in &c.zones {
        let _ = writeln!(out, "  zone {} {} {} {} {} {} {} {}", z.x, z.y, z.width, z.height, z.id, z.unknown_9, z.unknown_a, z.unknown_b);
    }
    for (name, list) in [("path", &c.paths), ("progress", &c.progress_paths)] {
        for p in list {
            let _ = writeln!(out, "  {name} {} {} nodes={}", p.id, p.unknown, p.nodes.len());
            for n in &p.nodes {
                let _ = writeln!(out, "    node {} {} {} {} {} {}", n.x, n.y, n.speed.raw(), n.acceleration.raw(), n.delay, n.flags);
            }
        }
    }
    let _ = write!(out, "  banks");
    for bank in c.object_banks {
        let _ = write!(out, " {bank}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "  rewrite course={} bgdat={}", hashed(&level.serialize_course()), hashed(&level.serialize_bgdat()));

    let references = level.tileset_references();
    let resources = match resolve_level_graphics(&mut files.borrow_mut(), references) {
        Ok(resources) => resources,
        Err(error) => {
            let _ = writeln!(out, "  graphics {}", failed(&error));
            return;
        }
    };
    let tilesets = load_level_tilesets(&resources.tilesets, references.tileset_id, Some(overlays));
    for (slot, (tileset, error)) in tilesets.tilesets.iter().zip(&tilesets.errors).enumerate() {
        let mut pixels = Sha256::new();
        for block in &tileset.map16_tiles {
            pixels.update(sha_image(block));
        }
        let _ = writeln!(
            out,
            "  tileset {slot} blocks={} objects={} pixels={}{}",
            tileset.map16_tiles.len(),
            tileset.objects.len(),
            hex(pixels),
            if error.is_empty() { "" } else { " error" }
        );
    }
    let [first, second, third] = &tilesets.tilesets;
    let tilemap = rasterize_level_objects(level.objects(), &[Some(first), Some(second), Some(third)]);
    let mut cells = Sha256::new();
    for chunk in &tilemap.chunks {
        cells.update(chunk.x.to_le_bytes());
        cells.update(chunk.y.to_le_bytes());
        for cell in &chunk.cells {
            cells.update(cell.block.to_le_bytes());
            cells.update([cell.slot]);
        }
    }
    let _ = writeln!(out, "  tilemap chunks={} cells={}", tilemap.chunks.len(), hex(cells));
    let image = |result: Result<RgbaImage>| match result {
        Ok(image) => sha_image(&image),
        Err(error) => failed(&error),
    };
    let _ = writeln!(
        out,
        "  background {} foreground {}",
        image(load_background_graphics(&resources.background)),
        image(load_background_graphics(&resources.foreground))
    );
}

fn draw_kind(kind: LuaActorDrawKind) -> &'static str {
    match kind {
        LuaActorDrawKind::Image => "image",
        LuaActorDrawKind::Text => "text",
        LuaActorDrawKind::FillRect => "fillRect",
        LuaActorDrawKind::StrokeRect => "strokeRect",
        LuaActorDrawKind::FillEllipse => "fillEllipse",
        LuaActorDrawKind::Line => "line",
    }
}

fn bounds_text(bounds: &Result<Option<SpriteBounds>>) -> String {
    match bounds {
        Err(error) => failed(error),
        Ok(None) => "none".to_owned(),
        Ok(Some(b)) => format!("{},{},{},{}", b.x, b.y, b.width, b.height),
    }
}

/// The stills are opaque at the size their header declares: the oracle compares geometry, and a PNG codec of its own
/// would only add a third implementation.
fn oracle_png(bytes: &[u8]) -> Result<RgbaImage> {
    if bytes.len() < 24 || bytes.get(1..4) != Some(b"PNG") {
        return Err(Error::new(ErrorCode::ParseError, "not a PNG"));
    }
    let read = |at: usize| bytes.get(at..at + 4).and_then(|b| b.try_into().ok()).map_or(0, u32::from_be_bytes) as usize;
    let (width, height) = (read(16), read(20));
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(Error::new(ErrorCode::ParseError, "bad PNG size"));
    }
    Ok(RgbaImage { width, height, pixels: vec![0xff; width * height * 4] })
}

pub fn oracle(project_dir: &Path, data_dir: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let manifest = match load_manifest(&project_dir.join(MANIFEST_FILE_NAME)) {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = writeln!(err, "{}", error.message);
            return 2;
        }
    };
    let Some(rom) = manifest.rom() else {
        let _ = writeln!(err, "the project has no ROM");
        return 2;
    };
    let reader = match RomReader::open(project_dir.join(&rom.rom_path)) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = writeln!(err, "{}", error.message);
            return 2;
        }
    };
    let mut filesystem = EffectiveFilesystem::new();
    filesystem.set_rom(Some(reader));
    let files = RefCell::new(filesystem);
    let rom_files = files.borrow().rom().map(|rom| rom.files().to_vec()).unwrap_or_default();
    let game_code = files.borrow().rom().map(|rom| rom.header().game_code.clone()).unwrap_or_default();

    let _ = writeln!(out, "rom {game_code} files={}", rom_files.len());
    for file in &rom_files {
        let data = files.borrow_mut().rom_mut().map(|rom| rom.read_id(file.id));
        match data {
            Some(Ok(data)) => report_file(out, "", &format!("file {} {}", file.id, file.path), &file.path, &data, true),
            Some(Err(error)) => {
                let _ = writeln!(out, "file {} {} {}", file.id, file.path, failed(&error));
            }
            None => {}
        }
    }
    let mut id = 0u32;
    while let Some(Ok(overlay)) = files.borrow_mut().rom_mut().map(|rom| rom.read_overlay(id)) {
        let _ = writeln!(out, "overlay9 {id} raw={} repack={}", sha(&overlay), hashed(&compress_blz_overlay(&overlay)));
        id += 1;
    }
    if let Some(Ok(banner)) = files.borrow_mut().rom_mut().map(RomReader::read_banner) {
        let _ = writeln!(out, "banner {}", round_trip(parse_ds_banner(&banner), write_ds_banner));
    }

    let actors_dir = data_dir.join("actors");
    let resolve = |kind: ActorResourceKind, name: &str| -> Result<ActorResource> {
        if kind == ActorResourceKind::Image {
            let path = actors_dir.join(name);
            let data = read_file(&path).map_err(|_| Error::new(ErrorCode::NotFound, "missing"))?;
            return Ok(ActorResource { rom_path: name.to_owned(), source: path, data });
        }
        let data = files.borrow_mut().read(name).map_err(|e| Error::new(ErrorCode::NotFound, e.message))?;
        Ok(ActorResource { rom_path: name.to_owned(), source: Default::default(), data })
    };
    let overlays = match load_lua_tile_layers(&[LuaTileLayer { name: "shipped".into(), directory: data_dir.join("tiles") }], &resolve) {
        Ok(overlays) => overlays,
        Err(error) => {
            let _ = writeln!(err, "{}", error.message);
            return 3;
        }
    };
    for script in &overlays.scripts {
        let textured = script.blocks.values().filter(|overlay| overlay.image.is_some()).count();
        let _ = writeln!(out, "tiles {} blocks={} textured={textured} errors={}", script.name, script.blocks.len(), script.errors.len());
    }

    let paths: Vec<&str> = rom_files.iter().map(|file| file.path.as_str()).collect();
    for path in &paths {
        if !path.starts_with("course/") || !path.ends_with(".bin") || path.ends_with("_bgdat.bin") {
            continue;
        }
        let bgdat = format!("{}_bgdat.bin", path.trim_end_matches(".bin"));
        if !paths.contains(&bgdat.as_str()) {
            continue;
        }
        report_level(out, &files, path, &bgdat, &overlays);
    }

    let actors =
        match load_lua_actor_layers(&[LuaActorLayer { name: "shipped".into(), directory: actors_dir.clone() }], &resolve, Some(&oracle_png), None) {
            Ok(actors) => actors,
            Err(error) => {
                let _ = writeln!(err, "{}", error.message);
                return 3;
            }
        };
    for actor in &actors.actors {
        let _ = writeln!(
            out,
            "actor {} object={} fields={} init={} renderer={}",
            actor.source.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            actor.object_id.map_or("-".to_owned(), |id| id.to_string()),
            actor.sprite.fields.len(),
            if actor.init_error.is_empty() { "ok" } else { "error" },
            u8::from(has_lua_renderer(actor))
        );
        let input = LuaActorRenderInput { object_id: actor.sprite.object_id, hash: actor.hash, ..LuaActorRenderInput::new() };
        let _ = writeln!(
            out,
            "  bounds grab={} render={}",
            bounds_text(&lua_actor_bounds(actor, &input, false)),
            bounds_text(&lua_actor_bounds(actor, &input, true))
        );
        let rendered = match render_lua_actor(actor, &input) {
            Ok(rendered) => rendered,
            Err(error) => {
                let _ = writeln!(out, "  draw {}", failed(&error));
                continue;
            }
        };
        let _ = writeln!(out, "  draw custom={} commands={}", u8::from(rendered.custom_rendered), rendered.commands.len());
        for c in &rendered.commands {
            let _ = writeln!(
                out,
                "    {} src={},{},{},{} dst={},{},{},{} rot={} op={} end={},{} stroke={} color={:08x} flip={}{} text={} image={}",
                draw_kind(c.kind),
                c.source_x,
                c.source_y,
                c.source_width,
                c.source_height,
                number(c.x),
                number(c.y),
                number(c.width),
                number(c.height),
                number(c.rotation),
                number(c.opacity),
                number(c.x2),
                number(c.y2),
                number(c.stroke_width),
                c.color,
                u8::from(c.flip_x),
                u8::from(c.flip_y),
                c.text,
                c.image.as_ref().map_or("-".to_owned(), |image| format!("{}x{}", image.width, image.height))
            );
        }
    }
    0
}
