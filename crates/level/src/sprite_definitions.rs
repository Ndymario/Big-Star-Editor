//! Sprite (actor) definitions and the bit fields of sprite settings. Port of
//! `libs/nsmb-project/src/sprite_definitions.cpp`; pure, so the frontend can generate setting fields itself.

use serde::{Deserialize, Serialize};

/// An actor that defines no bounds function grabs as the 16x16 tile it sits on, the same default NSMBStageObj.cs
/// starts its switch from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteBounds {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Default for SpriteBounds {
    fn default() -> Self {
        Self { x: 0, y: 0, width: 16, height: 16 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteFieldChoice {
    pub value: u32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteFieldDefinition {
    pub key: String,
    pub label: String,
    pub bit_offset: u8,
    pub bit_width: u8,
    pub choices: Vec<SpriteFieldChoice>,
    pub notes: String,
    pub value_offset: i32,
    pub checkbox_value: u32,
    pub signed_value: bool,
}

impl Default for SpriteFieldDefinition {
    fn default() -> Self {
        Self {
            key: String::new(),
            label: String::new(),
            bit_offset: 0,
            bit_width: 1,
            choices: Vec::new(),
            notes: String::new(),
            value_offset: 0,
            checkbox_value: 0,
            signed_value: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteDefinition {
    pub object_id: u32,
    pub hash: Option<u32>,
    pub name: String,
    pub category: String,
    pub notes: String,
    /// The actor's bounds functions evaluated once at zero settings, kept as an offset from the actor origin so hit
    /// testing and thumbnails have a cheap default without calling into Lua.
    pub grab_bounds: SpriteBounds,
    pub render_bounds: SpriteBounds,
    pub fields: Vec<SpriteFieldDefinition>,
}

impl Default for SpriteDefinition {
    fn default() -> Self {
        Self {
            object_id: 0,
            hash: None,
            name: String::new(),
            category: "Other".to_owned(),
            notes: String::new(),
            grab_bounds: SpriteBounds::default(),
            render_bounds: SpriteBounds::default(),
            fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteDefinitionCatalog {
    pub definitions: Vec<SpriteDefinition>,
}

impl SpriteDefinitionCatalog {
    pub fn by_object_id(&self, object_id: u32) -> Option<&SpriteDefinition> {
        self.definitions.iter().find(|definition| definition.object_id == object_id)
    }

    pub fn by_hash(&self, hash: u32) -> Option<&SpriteDefinition> {
        self.definitions.iter().find(|definition| definition.hash == Some(hash))
    }
}

/// `1 << shift` as an `i64`, saturating where the C++ shift would be undefined (a width of 0 or past 63 bits).
fn power(shift: i64) -> i64 {
    u32::try_from(shift).ok().and_then(|shift| 1i64.checked_shl(shift)).unwrap_or(0)
}

fn value_mask(field: &SpriteFieldDefinition) -> u64 {
    1u64.checked_shl(u32::from(field.bit_width)).map_or(u64::MAX, |bit| bit - 1)
}

pub fn sprite_field_min(field: &SpriteFieldDefinition) -> i32 {
    if field.checkbox_value != 0 {
        return 0;
    }
    let minimum = if field.signed_value { -power(i64::from(field.bit_width) - 1) } else { 0 };
    (minimum + i64::from(field.value_offset)) as i32
}

pub fn sprite_field_max(field: &SpriteFieldDefinition) -> i32 {
    if field.checkbox_value != 0 {
        return 1;
    }
    let maximum = if field.signed_value { power(i64::from(field.bit_width) - 1) - 1 } else { value_mask(field) as i64 };
    maximum.wrapping_add(i64::from(field.value_offset)) as i32
}

pub fn read_sprite_field(data: u64, field: &SpriteFieldDefinition) -> i32 {
    let raw = data.checked_shr(u32::from(field.bit_offset)).unwrap_or(0) & value_mask(field);
    if field.checkbox_value != 0 {
        return (raw / u64::from(field.checkbox_value)) as i32;
    }
    let mut value = raw as i64;
    let sign = power(i64::from(field.bit_width) - 1) as u64;
    if field.signed_value && raw & sign != 0 {
        value = value.wrapping_sub(power(i64::from(field.bit_width)));
    }
    value.wrapping_add(i64::from(field.value_offset)) as i32
}

pub fn write_sprite_field(data: u64, field: &SpriteFieldDefinition, value: i32) -> u64 {
    let mask = value_mask(field);
    let raw = if field.checkbox_value != 0 {
        i64::from(value).wrapping_mul(i64::from(field.checkbox_value))
    } else {
        i64::from(value) - i64::from(field.value_offset)
    };
    let shift = u32::from(field.bit_offset);
    let clear = mask.checked_shl(shift).unwrap_or(0);
    (data & !clear) | ((raw as u64) & mask).checked_shl(shift).unwrap_or(0)
}
