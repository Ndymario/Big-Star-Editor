//! TEX0 textures inside NSBTX/NSBMD files (or standalone), their rendering, and the 4x4 texel compressor. Port of
//! `libs/nsmb-formats/src/tex0.cpp`.
//!
//! Offsets are worked out in `u64` so a hostile header cannot overflow them on 32-bit targets.

use std::collections::BTreeMap;

use bigstar_core::{Error, ErrorCode, Result};

use crate::g2d::{put16, put32, u16_at, u32_at};
use crate::graphics::unpack_bgr555;
use crate::{Bgr555Color, RgbaImage};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureFormat {
    #[default]
    None,
    A3I5,
    Palette4,
    Palette16,
    Palette256,
    Compressed4x4,
    A5I3,
    Direct,
}

impl TextureFormat {
    fn from_bits(bits: u32) -> Self {
        match bits & 7 {
            1 => Self::A3I5,
            2 => Self::Palette4,
            3 => Self::Palette16,
            4 => Self::Palette256,
            5 => Self::Compressed4x4,
            6 => Self::A5I3,
            7 => Self::Direct,
            _ => Self::None,
        }
    }

    fn bits_per_pixel(self) -> u64 {
        match self {
            Self::A3I5 | Self::Palette256 | Self::A5I3 => 8,
            Self::Palette4 | Self::Compressed4x4 => 2,
            Self::Palette16 => 4,
            Self::Direct => 16,
            Self::None => 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tex0Texture {
    pub name: String,
    pub format: TextureFormat,
    pub width: usize,
    pub height: usize,
    pub original_width: usize,
    pub original_height: usize,
    pub color_zero_transparent: bool,
    pub texels: Vec<u8>,
    pub palette_indices: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tex0Palette {
    pub name: String,
    pub flags: u16,
    pub colors: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tex0 {
    /// The complete NSBTX/NSBMD, or a standalone TEX0 block. Writers preserve every non-texture byte and require the
    /// same texture/palette layout.
    pub original: Vec<u8>,
    pub textures: Vec<Tex0Texture>,
    pub palettes: Vec<Tex0Palette>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tex4x4Encoded {
    pub texels: Vec<u8>,
    pub palette_indices: Vec<u8>,
    pub palette: Vec<u8>,
}

fn err(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

fn fits(data: &[u8], offset: u64, size: u64) -> bool {
    let len = data.len() as u64;
    offset <= len && size <= len - offset
}

fn at(offset: u64) -> usize {
    usize::try_from(offset).unwrap_or(usize::MAX)
}

fn r16(data: &[u8], offset: u64) -> u64 {
    u64::from(u16_at(data, at(offset)))
}

fn r32(data: &[u8], offset: u64) -> u64 {
    u64::from(u32_at(data, at(offset)))
}

fn bytes(data: &[u8], offset: u64, size: u64) -> Vec<u8> {
    let end = offset.saturating_add(size);
    data.get(at(offset)..at(end)).unwrap_or_default().to_vec()
}

fn magic(data: &[u8], offset: u64, value: &[u8]) -> bool {
    fits(data, offset, value.len() as u64) && data.get(at(offset)..).is_some_and(|rest| rest.starts_with(value))
}

fn name16(data: &[u8], offset: u64) -> String {
    let field = data.get(at(offset)..at(offset + 16)).unwrap_or_default();
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(field.get(..end).unwrap_or_default()).into_owned()
}

struct DictionaryEntry {
    name: String,
    definition: u64,
}

fn dictionary(data: &[u8], offset: u64, limit: u64, unit: u64) -> Result<Vec<DictionaryEntry>> {
    if !fits(data, offset, 8) || offset + 8 > limit {
        return Err(err(ErrorCode::OutOfBounds, "TEX0 dictionary header is outside its block"));
    }
    let count = u64::from(data.get(at(offset + 1)).copied().unwrap_or(0));
    let size = r16(data, offset + 2);
    let entry_offset = r16(data, offset + 6);
    if size < 8 + (count + 1) * 4 || offset + size > limit {
        return Err(err(ErrorCode::ParseError, "TEX0 dictionary size is invalid"));
    }
    let entry = offset + entry_offset;
    if !fits(data, entry, 4) || entry + 4 > offset + size || r16(data, entry) != unit {
        return Err(err(ErrorCode::ParseError, "TEX0 dictionary entry header is invalid"));
    }
    let names = entry + r16(data, entry + 2);
    if !fits(data, entry + 4, count * unit)
        || !fits(data, names, count * 16)
        || entry + 4 + count * unit > offset + size
        || names + count * 16 > offset + size
    {
        return Err(err(ErrorCode::OutOfBounds, "TEX0 dictionary entries are outside their block"));
    }
    Ok((0..count).map(|index| DictionaryEntry { name: name16(data, names + index * 16), definition: entry + 4 + index * unit }).collect())
}

#[derive(Default)]
struct Layout {
    value: Tex0,
    texture_definitions: Vec<u64>,
    texture_data: Vec<u64>,
    texture_indices: Vec<u64>,
    palette_definitions: Vec<u64>,
    palette_data: Vec<u64>,
}

fn find_tex0(data: &[u8]) -> Result<u64> {
    if magic(data, 0, b"TEX0") {
        if !fits(data, 0, 8) || r32(data, 4) != data.len() as u64 {
            return Err(err(ErrorCode::ParseError, "standalone TEX0 size is invalid"));
        }
        return Ok(0);
    }
    if data.len() < 16 || (!magic(data, 0, b"BTX0") && !magic(data, 0, b"BMD0")) {
        return Err(err(ErrorCode::ParseError, "input is not NSBTX, NSBMD, or TEX0"));
    }
    if r16(data, 4) != 0xfeff || r32(data, 8) != data.len() as u64 {
        return Err(err(ErrorCode::ParseError, "Nitro G3D header is invalid"));
    }
    let blocks = r16(data, 14);
    if !fits(data, 16, blocks * 4) {
        return Err(err(ErrorCode::OutOfBounds, "Nitro G3D block table is truncated"));
    }
    for index in 0..blocks {
        let offset = r32(data, 16 + index * 4);
        if !fits(data, offset, 8) {
            return Err(err(ErrorCode::OutOfBounds, "Nitro G3D block offset is outside the file"));
        }
        let size = r32(data, offset + 4);
        if size < 8 || !fits(data, offset, size) {
            return Err(err(ErrorCode::ParseError, "Nitro G3D block size is invalid"));
        }
        if magic(data, offset, b"TEX0") {
            return Ok(offset);
        }
    }
    Err(err(ErrorCode::NotFound, "NSBTX/NSBMD has no TEX0 block"))
}

fn layout(data: &[u8]) -> Result<Layout> {
    let base = find_tex0(data)?;
    let size = r32(data, base + 4);
    let end = base + size;
    if size < 0x3c {
        return Err(err(ErrorCode::ParseError, "TEX0 header is truncated"));
    }
    let mut out = Layout { value: Tex0 { original: data.to_vec(), ..Tex0::default() }, ..Layout::default() };
    let texture_region = base + r32(data, base + 0x14);
    let texture_region_size = r16(data, base + 0x0c) * 8;
    let compressed_region = base + r32(data, base + 0x24);
    let compressed_region_size = r16(data, base + 0x1c) * 8;
    let index_region = base + r32(data, base + 0x28);
    let palette_region = base + r32(data, base + 0x38);
    let palette_region_size = r16(data, base + 0x30) * 8;
    if texture_region + texture_region_size > end
        || compressed_region + compressed_region_size > end
        || index_region + compressed_region_size / 2 > end
        || palette_region + palette_region_size > end
    {
        return Err(err(ErrorCode::OutOfBounds, "TEX0 data region is outside its block"));
    }

    for entry in dictionary(data, base + r16(data, base + 0x0e), end, 8)? {
        let parameter = r32(data, entry.definition);
        let extra = r32(data, entry.definition + 4);
        let format = TextureFormat::from_bits((parameter >> 26) as u32);
        let width = 8u64 << ((parameter >> 20) & 7);
        let height = 8u64 << ((parameter >> 23) & 7);
        let bpp = format.bits_per_pixel();
        if bpp == 0 {
            return Err(err(ErrorCode::Unsupported, "TEX0 texture has format 0"));
        }
        let size = width * height * bpp / 8;
        let address = (parameter & 0xffff) * 8;
        let compressed = format == TextureFormat::Compressed4x4;
        let data_offset = if compressed { compressed_region } else { texture_region } + address;
        let region_end = if compressed { compressed_region + compressed_region_size } else { texture_region + texture_region_size };
        if data_offset + size > region_end {
            return Err(err(ErrorCode::OutOfBounds, "TEX0 texture data is outside its region"));
        }
        let original_width = extra & 0x7ff;
        let original_height = (extra >> 11) & 0x7ff;
        let mut texture = Tex0Texture {
            name: entry.name,
            format,
            width: width as usize,
            height: height as usize,
            original_width: if original_width == 0 { width } else { original_width } as usize,
            original_height: if original_height == 0 { height } else { original_height } as usize,
            color_zero_transparent: parameter & (1 << 29) != 0,
            texels: bytes(data, data_offset, size),
            palette_indices: Vec::new(),
        };
        out.texture_definitions.push(entry.definition);
        out.texture_data.push(data_offset);
        let mut indices = 0;
        if compressed {
            let index_offset = index_region + address / 2;
            let index_size = width * height / 8;
            if index_offset + index_size > index_region + compressed_region_size / 2 {
                return Err(err(ErrorCode::OutOfBounds, "TEX0 4x4 palette-index data is outside its region"));
            }
            texture.palette_indices = bytes(data, index_offset, index_size);
            indices = index_offset;
        }
        out.texture_indices.push(indices);
        out.value.textures.push(texture);
    }

    let palettes = dictionary(data, base + r16(data, base + 0x34), end, 4)?;
    let mut offsets = Vec::with_capacity(palettes.len());
    for entry in &palettes {
        let offset = r16(data, entry.definition) * 8;
        if offset > palette_region_size {
            return Err(err(ErrorCode::OutOfBounds, "TEX0 palette offset is outside its region"));
        }
        offsets.push(offset);
    }
    for (entry, &offset) in palettes.into_iter().zip(&offsets) {
        let next = offsets.iter().copied().filter(|&candidate| candidate > offset).fold(palette_region_size, u64::min);
        out.value.palettes.push(Tex0Palette {
            flags: r16(data, entry.definition + 2) as u16,
            colors: bytes(data, palette_region + offset, next - offset),
            name: entry.name,
        });
        out.palette_definitions.push(entry.definition);
        out.palette_data.push(palette_region + offset);
    }
    Ok(out)
}

pub fn parse_tex0(data: &[u8]) -> Result<Tex0> {
    Ok(layout(data)?.value)
}

fn overwrite(output: &mut [u8], offset: u64, source: &[u8]) {
    if let Some(slot) = output.get_mut(at(offset)..at(offset.saturating_add(source.len() as u64))) {
        slot.copy_from_slice(source);
    }
}

pub fn write_tex0(value: &Tex0) -> Result<Vec<u8>> {
    if value.original.is_empty() {
        return Err(err(ErrorCode::InvalidArgument, "TEX0 writer needs a parsed NSBTX/NSBMD layout"));
    }
    let baseline = layout(&value.original)?;
    if value.textures.len() != baseline.value.textures.len() || value.palettes.len() != baseline.value.palettes.len() {
        return Err(err(ErrorCode::InvalidArgument, "TEX0 writer cannot add or remove definitions"));
    }
    let mut output = value.original.clone();
    for (index, (edited, original)) in value.textures.iter().zip(&baseline.value.textures).enumerate() {
        let same_shape = edited.name == original.name
            && edited.format == original.format
            && edited.width == original.width
            && edited.height == original.height
            && edited.original_width == original.original_width
            && edited.original_height == original.original_height
            && edited.texels.len() == original.texels.len()
            && edited.palette_indices.len() == original.palette_indices.len();
        if !same_shape {
            return Err(err(ErrorCode::InvalidArgument, "TEX0 writer preserves texture names, formats, and dimensions"));
        }
        let data = baseline.texture_data.get(index).copied().unwrap_or(0);
        overwrite(&mut output, data, &edited.texels);
        if !edited.palette_indices.is_empty() {
            overwrite(&mut output, baseline.texture_indices.get(index).copied().unwrap_or(0), &edited.palette_indices);
        }
        let definition = at(baseline.texture_definitions.get(index).copied().unwrap_or(0));
        let parameter = u32_at(&output, definition);
        let parameter = if edited.color_zero_transparent { parameter | (1 << 29) } else { parameter & !(1 << 29) };
        put32(&mut output, definition, parameter);
    }
    for (index, (edited, original)) in value.palettes.iter().zip(&baseline.value.palettes).enumerate() {
        if edited.name != original.name || edited.colors.len() != original.colors.len() {
            return Err(err(ErrorCode::InvalidArgument, "TEX0 writer preserves palette names and sizes"));
        }
        overwrite(&mut output, baseline.palette_data.get(index).copied().unwrap_or(0), &edited.colors);
        let definition = at(baseline.palette_definitions.get(index).copied().unwrap_or(0));
        put16(&mut output, definition + 2, edited.flags);
    }
    Ok(output)
}

fn color(palette: &[u8], index: usize) -> Bgr555Color {
    if index * 2 + 1 >= palette.len() {
        return Bgr555Color::default();
    }
    let packed = u16_at(palette, index * 2);
    unpack_bgr555(packed, false)
}

fn blend(a: Bgr555Color, b: Bgr555Color, wa: u32, wb: u32, divisor: u32) -> Bgr555Color {
    let mix = |x: u8, y: u8| ((u32::from(x) * wa + u32::from(y) * wb) / divisor) as u8;
    Bgr555Color::new(mix(a.red, b.red), mix(a.green, b.green), mix(a.blue, b.blue), false)
}

fn pack(red: u8, green: u8, blue: u8) -> u16 {
    crate::graphics::pack_bgr555(red, green, blue)
}

fn difference(a: u16, b: u16) -> i32 {
    let channel = |value: u16, shift: u16| i32::from((value >> shift) & 31);
    [0, 5, 10].iter().map(|&shift| (channel(a, shift) - channel(b, shift)).pow(2)).sum()
}

fn set_pixel(image: &mut RgbaImage, index: usize, value: Bgr555Color, alpha: u8) {
    if let Some(slot) = image.pixels.get_mut(index * 4..index * 4 + 4) {
        slot.copy_from_slice(&[value.red, value.green, value.blue, alpha]);
    }
}

fn texel(texture: &Tex0Texture, index: usize, what: &str) -> Result<u8> {
    texture.texels.get(index).copied().ok_or_else(|| err(ErrorCode::OutOfBounds, what))
}

pub fn render_tex0_texture(texture: &Tex0Texture, palette: &Tex0Palette) -> Result<RgbaImage> {
    if texture.width == 0 || texture.height == 0 || texture.width > 1024 || texture.height > 1024 {
        return Err(err(ErrorCode::InvalidArgument, "TEX0 texture dimensions are invalid"));
    }
    if texture.original_width > texture.width || texture.original_height > texture.height {
        return Err(err(ErrorCode::InvalidArgument, "TEX0 original dimensions exceed their storage dimensions"));
    }
    let (width, height) = (texture.original_width, texture.original_height);
    let mut image = RgbaImage { width, height, pixels: vec![0; width * height * 4] };
    for y in 0..height {
        for x in 0..width {
            let source = y * texture.width + x;
            let target = y * width + x;
            let mut alpha = 255u8;
            let index = match texture.format {
                TextureFormat::A3I5 => {
                    let value = texel(texture, source, "TEX0 A3I5 data is truncated")?;
                    alpha = (u32::from(value >> 5) * 255 / 7) as u8;
                    usize::from(value & 31)
                }
                TextureFormat::A5I3 => {
                    let value = texel(texture, source, "TEX0 A5I3 data is truncated")?;
                    alpha = (u32::from(value >> 3) * 255 / 31) as u8;
                    usize::from(value & 7)
                }
                TextureFormat::Palette4 => usize::from((texel(texture, source / 4, "TEX0 4-color data is truncated")? >> ((source % 4) * 2)) & 3),
                TextureFormat::Palette16 => usize::from((texel(texture, source / 2, "TEX0 16-color data is truncated")? >> ((source % 2) * 4)) & 15),
                TextureFormat::Palette256 => usize::from(texel(texture, source, "TEX0 256-color data is truncated")?),
                TextureFormat::Direct => {
                    if source * 2 + 1 >= texture.texels.len() {
                        return Err(err(ErrorCode::OutOfBounds, "TEX0 direct-color data is truncated"));
                    }
                    let packed = u16_at(&texture.texels, source * 2);
                    let value = unpack_bgr555(packed, false);
                    set_pixel(&mut image, target, value, if packed & 0x8000 != 0 { 255 } else { 0 });
                    continue;
                }
                TextureFormat::Compressed4x4 => {
                    let blocks_wide = texture.width / 4;
                    let block = (y / 4) * blocks_wide + x / 4;
                    let row = block * 4 + y % 4;
                    if row >= texture.texels.len() || block * 2 + 1 >= texture.palette_indices.len() {
                        return Err(err(ErrorCode::OutOfBounds, "TEX0 4x4 data is truncated"));
                    }
                    let code = usize::from((texel(texture, row, "TEX0 4x4 data is truncated")? >> ((x % 4) * 2)) & 3);
                    let control = u16_at(&texture.palette_indices, block * 2);
                    let base = usize::from(control & 0x3fff) * 2;
                    let mode = control >> 14;
                    let (a, b) = (color(&palette.colors, base), color(&palette.colors, base + 1));
                    let chosen = match mode {
                        0 | 1 if code == 3 => {
                            set_pixel(&mut image, target, Bgr555Color::default(), 0);
                            continue;
                        }
                        0 | 2 => color(&palette.colors, base + code),
                        1 => {
                            if code < 2 {
                                color(&palette.colors, base + code)
                            } else {
                                blend(a, b, 1, 1, 2)
                            }
                        }
                        _ => match code {
                            0 | 1 => color(&palette.colors, base + code),
                            2 => blend(a, b, 5, 3, 8),
                            _ => blend(a, b, 3, 5, 8),
                        },
                    };
                    set_pixel(&mut image, target, chosen, 255);
                    continue;
                }
                TextureFormat::None => return Err(err(ErrorCode::Unsupported, "TEX0 texture format is unsupported")),
            };
            if index * 2 + 1 >= palette.colors.len() {
                return Err(err(ErrorCode::OutOfBounds, "TEX0 palette index is outside the selected palette"));
            }
            if texture.color_zero_transparent && index == 0 {
                alpha = 0;
            }
            set_pixel(&mut image, target, color(&palette.colors, index), alpha);
        }
    }
    Ok(image)
}

pub fn compress_tex4x4(image: &RgbaImage, maximum_palette_groups: usize) -> Result<Tex4x4Encoded> {
    let complete = image.width != 0
        && image.height != 0
        && image.width.is_multiple_of(4)
        && image.height.is_multiple_of(4)
        && image.width.checked_mul(image.height).and_then(|n| n.checked_mul(4)) == Some(image.pixels.len());
    if !complete {
        return Err(err(ErrorCode::InvalidArgument, "4x4 compression needs a complete RGBA image with dimensions divisible by four"));
    }
    if maximum_palette_groups == 0 || maximum_palette_groups > 8192 {
        return Err(err(ErrorCode::InvalidArgument, "4x4 palette-group limit must be 1 through 8192"));
    }
    let mut output = Tex4x4Encoded::default();
    let mut groups: Vec<[u16; 4]> = Vec::new();
    for by in (0..image.height).step_by(4) {
        for bx in (0..image.width).step_by(4) {
            let mut transparent = false;
            let mut frequency: BTreeMap<u16, usize> = BTreeMap::new();
            let mut source = [0u16; 16];
            let mut clear = [false; 16];
            for y in 0..4 {
                for x in 0..4 {
                    let p = ((by + y) * image.width + bx + x) * 4;
                    let Some(&[red, green, blue, alpha]) = image.pixels.get(p..p + 4).and_then(|s| <&[u8; 4]>::try_from(s).ok()) else { continue };
                    let b = y * 4 + x;
                    let is_clear = alpha < 128;
                    if let Some(slot) = clear.get_mut(b) {
                        *slot = is_clear;
                    }
                    if is_clear {
                        transparent = true;
                    } else {
                        let packed = pack(red, green, blue);
                        if let Some(slot) = source.get_mut(b) {
                            *slot = packed;
                        }
                        *frequency.entry(packed).or_default() += 1;
                    }
                }
            }
            let mut ranked: Vec<(u16, usize)> = frequency.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let usable = if transparent { 3 } else { 4 };
            let mut group = [0u16; 4];
            for (slot, &(packed, _)) in group.iter_mut().zip(&ranked).take(usable) {
                *slot = packed;
            }
            if let Some(&(most, _)) = ranked.first() {
                for slot in group.iter_mut().take(usable).skip(ranked.len()) {
                    *slot = most;
                }
            }
            let nearest = |group: &[u16; 4], value: u16| group.iter().take(usable).map(|&c| difference(value, c)).min().unwrap_or(i32::MAX);
            let group_index = if let Some(found) = groups.iter().position(|g| *g == group) {
                found
            } else if groups.len() < maximum_palette_groups {
                groups.push(group);
                groups.len() - 1
            } else {
                // Nearest existing group is enough until import QA demonstrates a need for global palette clustering.
                let mut best = i32::MAX;
                let mut best_index = 0;
                for (candidate, existing) in groups.iter().enumerate() {
                    let error: i32 = source.iter().zip(&clear).filter(|(_, c)| !**c).map(|(&s, _)| nearest(existing, s)).sum();
                    if error < best {
                        best = error;
                        best_index = candidate;
                    }
                }
                group = groups.get(best_index).copied().unwrap_or_default();
                best_index
            };
            let mut rows = [0u8; 4];
            for (y, row) in rows.iter_mut().enumerate() {
                for x in 0..4 {
                    let i = y * 4 + x;
                    let mut code = 3usize;
                    if !clear.get(i).copied().unwrap_or(true) {
                        let value = source.get(i).copied().unwrap_or(0);
                        let mut best = i32::MAX;
                        for (c, &candidate) in group.iter().enumerate().take(usable) {
                            let error = difference(value, candidate);
                            if error < best {
                                best = error;
                                code = c;
                            }
                        }
                    }
                    *row |= (code << (x * 2)) as u8;
                }
            }
            output.texels.extend_from_slice(&rows);
            let control = (group_index * 2) as u16 | ((u16::from(!transparent) * 2) << 14);
            output.palette_indices.extend_from_slice(&control.to_le_bytes());
        }
    }
    output.palette = groups.iter().flatten().flat_map(|packed| packed.to_le_bytes()).collect();
    Ok(output)
}
