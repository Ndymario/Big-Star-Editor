//! The module data catalogs NCPatcher generates (`level_data.json`, `stageobjects.json`). Reading them from the
//! project directory belongs to the project crate; only the pure parsing lives here.

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::fnv1a_32;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const DEFAULT_STAGE_OBJECT_ID_BASE: u16 = 326;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelDataKind {
    #[default]
    Flag,
    Value,
    Array,
    SizedArray,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelDataDeclaration {
    pub key: String,
    pub module: String,
    pub type_name: String,
    pub kind: LevelDataKind,
    pub hash: u32,
    pub array_size: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageObjectDeclaration {
    pub name: String,
    pub module: String,
    pub component: String,
    pub object: String,
    pub variant: String,
    pub info: String,
    pub header: String,
    pub hash: u32,
    pub object_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleDataCatalog {
    pub level_data: Vec<LevelDataDeclaration>,
    pub stage_object_id_base: u16,
    pub stage_objects: Vec<StageObjectDeclaration>,
}

impl Default for ModuleDataCatalog {
    fn default() -> Self {
        Self { level_data: Vec::new(), stage_object_id_base: DEFAULT_STAGE_OBJECT_ID_BASE, stage_objects: Vec::new() }
    }
}

impl ModuleDataCatalog {
    pub fn level_data_by_hash(&self, hash: u32) -> Option<&LevelDataDeclaration> {
        self.level_data.iter().find(|d| d.hash == hash)
    }

    pub fn stage_object_by_hash(&self, hash: u32) -> Option<&StageObjectDeclaration> {
        self.stage_objects.iter().find(|d| d.hash == hash)
    }
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

fn document(text: &str, name: &str) -> Result<Map<String, Value>> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(parse_error(format!("{name} root is not an object"))),
        Err(error) => Err(parse_error(format!("invalid JSON: {error}"))),
    }
}

fn schema(root: &Map<String, Value>, expected: &str, name: &str) -> Result<()> {
    match root.get("schema").and_then(Value::as_str) {
        Some(found) if found == expected => Ok(()),
        _ => Err(Error::new(ErrorCode::Unsupported, format!("{name} has an unsupported schema"))),
    }
}

fn only(node: &Map<String, Value>, allowed: &[&str], context: &str) -> Result<()> {
    match node.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(parse_error(format!("{context} contains unknown property {key}"))),
        None => Ok(()),
    }
}

fn hash_value(node: Option<&Value>, context: &str) -> Result<u32> {
    let text = node.and_then(Value::as_str).ok_or_else(|| parse_error(format!("{context} hash is not a string")))?;
    let digits = match text.strip_prefix("0x") {
        Some(digits) if digits.len() == 8 => digits,
        _ => return Err(parse_error(format!("{context} hash is not 0x plus eight hex digits"))),
    };
    if !digits.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        return Err(parse_error(format!("{context} hash must use lowercase hexadecimal")));
    }
    u32::from_str_radix(digits, 16).map_err(|_| parse_error(format!("{context} hash is invalid")))
}

fn required_string(node: &Map<String, Value>, key: &str, context: &str) -> Result<String> {
    match node.get(key).and_then(Value::as_str) {
        Some(text) if !text.is_empty() => Ok(text.to_owned()),
        _ => Err(parse_error(format!("{context} is missing {key}"))),
    }
}

fn entries<'a>(root: &'a Map<String, Value>, key: &str, name: &str) -> Result<&'a [Value]> {
    root.get(key).and_then(Value::as_array).map(Vec::as_slice).ok_or_else(|| parse_error(format!("{name} has no {key} array")))
}

pub fn parse_level_data_catalog(json: &str) -> Result<Vec<LevelDataDeclaration>> {
    let root = document(json, "level-data catalog")?;
    only(&root, &["schema", "levelData"], "level-data catalog")?;
    schema(&root, "nsmbtool.leveldata/1", "level-data catalog")?;
    let mut out = Vec::new();
    for node in entries(&root, "levelData", "level-data catalog")? {
        let node = node.as_object().ok_or_else(|| parse_error("level-data entry is not an object"))?;
        only(node, &["key", "module", "dataType", "hash", "typeName", "arraySize"], "level-data entry")?;
        let key = required_string(node, "key", "level-data entry")?;
        let module = required_string(node, "module", &key)?;
        let hash = hash_value(node.get("hash"), &key)?;
        if hash != fnv1a_32(&key) {
            return Err(parse_error(format!("{key} does not match its hash")));
        }
        let kind = match required_string(node, "dataType", &key)?.as_str() {
            "flag" => LevelDataKind::Flag,
            "value" => LevelDataKind::Value,
            "array" => LevelDataKind::Array,
            "sizedArray" => LevelDataKind::SizedArray,
            _ => return Err(parse_error(format!("{key} has an unknown dataType"))),
        };
        let type_name = if kind == LevelDataKind::Flag { String::new() } else { required_string(node, "typeName", &key)? };
        let array_size = if kind == LevelDataKind::Array {
            let count = node.get("arraySize").and_then(Value::as_i64).filter(|count| *count >= 1);
            let count = count.and_then(|count| usize::try_from(count).ok());
            Some(count.ok_or_else(|| parse_error(format!("{key} has an invalid arraySize")))?)
        } else {
            None
        };
        out.push(LevelDataDeclaration { key, module, type_name, kind, hash, array_size });
    }
    Ok(out)
}

pub fn parse_stage_object_catalog(json: &str) -> Result<(u16, Vec<StageObjectDeclaration>)> {
    let root = document(json, "stage-object catalog")?;
    only(&root, &["schema", "stageObjectIdBase", "stageObjects"], "stage-object catalog")?;
    schema(&root, "nsmbtool.stageobjects/1", "stage-object catalog")?;
    if root.get("stageObjectIdBase").and_then(Value::as_i64) != Some(i64::from(DEFAULT_STAGE_OBJECT_ID_BASE)) {
        return Err(parse_error("stage-object catalog has an invalid ID base"));
    }
    let mut out = Vec::new();
    for node in entries(&root, "stageObjects", "stage-object catalog")? {
        let node = node.as_object().ok_or_else(|| parse_error("stage-object entry is not an object"))?;
        only(node, &["name", "hash", "module", "component", "object", "variant", "objectId", "info", "header"], "stage-object entry")?;
        let name = required_string(node, "name", "stage-object entry")?;
        let hash = hash_value(node.get("hash"), &name)?;
        if hash != fnv1a_32(&name) {
            return Err(parse_error(format!("{name} does not match its hash")));
        }
        let module = required_string(node, "module", &name)?;
        let component = required_string(node, "component", &name)?;
        let object = required_string(node, "object", &name)?;
        let variant = required_string(node, "variant", &name)?;
        let info = required_string(node, "info", &name)?;
        let header = required_string(node, "header", &name)?;
        let object_id = node
            .get("objectId")
            .and_then(Value::as_i64)
            .filter(|id| *id >= 386)
            .and_then(|id| u32::try_from(id).ok())
            .ok_or_else(|| parse_error(format!("{name} has an invalid objectId")))?;
        out.push(StageObjectDeclaration { name, module, component, object, variant, info, header, hash, object_id });
    }
    Ok((DEFAULT_STAGE_OBJECT_ID_BASE, out))
}
