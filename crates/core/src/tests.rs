use super::*;

struct SetValue {
    before: i32,
    after: i32,
}

impl SetValue {
    fn boxed(target: &i32, value: i32) -> Box<dyn Command<i32>> {
        Box::new(Self { before: *target, after: value })
    }
}

impl Command<i32> for SetValue {
    fn name(&self) -> &str {
        "Set value"
    }
    fn execute(&mut self, target: &mut i32) -> Result<()> {
        *target = self.after;
        Ok(())
    }
    fn undo(&mut self, target: &mut i32) -> Result<()> {
        *target = self.before;
        Ok(())
    }
}

#[test]
fn endian_round_trip_and_bounds() {
    let mut data = vec![0u8; 4];
    let mut writer = LittleEndianWriter::new(&mut data);
    writer.u16(0, 0x1234).unwrap();
    writer.u32(0, 0x89ab_cdef).unwrap();
    assert!(writer.u32(1, 0).is_err());
    let reader = LittleEndianReader::new(&data);
    assert_eq!(reader.u16(0).unwrap(), 0xcdef);
    assert_eq!(reader.u32(0).unwrap(), 0x89ab_cdef);
    assert!(reader.u32(1).is_err());
    assert!(reader.u32(usize::MAX).is_err());
}

#[test]
fn byte_buffer_access() {
    let mut buffer = ByteBuffer::new(vec![1, 2, 3]);
    assert_eq!(buffer.at(1).unwrap(), 2);
    assert!(buffer.at(3).is_err());
    assert_eq!(buffer.slice(1, 2).unwrap().len(), 2);
    assert!(buffer.slice(2, usize::MAX).is_err());
    buffer.write(1, &[9, 8]).unwrap();
    assert_eq!(buffer.at(1).unwrap(), 9);
    assert!(buffer.write(2, &[9, 8]).is_err());
    buffer.append(&[7]);
    assert_eq!(buffer.bytes(), &[1, 9, 8, 7]);
}

#[test]
fn command_history_tracks_dirty_undo_and_redo() {
    let mut value = 1;
    let mut history = CommandHistory::new();
    history.execute(&mut value, SetValue::boxed(&1, 2)).unwrap();
    assert!(value == 2 && history.can_undo() && history.dirty());
    history.mark_clean();
    assert!(!history.dirty());
    history.undo(&mut value).unwrap();
    assert!(value == 1 && history.can_redo() && history.dirty());
    history.redo(&mut value).unwrap();
    assert!(value == 2 && !history.dirty());
    history.undo(&mut value).unwrap();
    history.execute(&mut value, SetValue::boxed(&1, 3)).unwrap();
    assert!(value == 3 && !history.can_redo() && history.dirty());
    assert_eq!(history.undo_names(), vec!["Set value".to_owned(); 1]);
    assert!(history.redo(&mut value).is_err());
}

#[test]
fn overwriting_past_the_clean_point_stays_dirty_at_the_same_position() {
    let mut value = 0;
    let mut history = CommandHistory::new();
    history.execute(&mut value, SetValue::boxed(&0, 1)).unwrap();
    history.mark_clean();
    history.undo(&mut value).unwrap();
    history.execute(&mut value, SetValue::boxed(&0, 5)).unwrap();
    assert!(history.dirty());
    history.clear();
    assert!(!history.dirty() && !history.can_undo());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn atomic_write_replaces_and_leaves_no_temporary() {
    let dir = std::env::temp_dir().join("big-star-core-atomic-test");
    let output = dir.join("out.bin");
    write_file_atomically(&output, &[1, 2, 3]).unwrap();
    write_file_atomically(&output, &[4, 5]).unwrap();
    assert_eq!(read_file(&output).unwrap(), vec![4, 5]);
    assert_eq!(read_file_prefix(&output, 1).unwrap(), vec![4]);
    assert!(!dir.join("out.bin.bse-tmp").exists());
    write_text_file_atomically(&output, "hi").unwrap();
    assert_eq!(read_file(&output).unwrap(), b"hi");
    assert!(read_file(&dir.join("missing")).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn fx32_converts() {
    assert_eq!(Fx32::from_integer(2).raw(), 8192);
    assert_eq!(Fx32::from_raw(2048).value(), 0.5);
}

#[test]
fn json_objects_keep_document_order() {
    // NCPatcher lists build variants in declaration order inside a JSON object; a sorted map would reorder them.
    let doc: serde_json::Value = serde_json::from_str(r#"{"variants":{"en":1,"fr":2,"de":3,"it":4,"sp":5,"ja":6,"ko":7,"zh":8,"pt":9}}"#).unwrap();
    let keys: Vec<&str> = doc["variants"].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["en", "fr", "de", "it", "sp", "ja", "ko", "zh", "pt"]);
    assert!(serde_json::from_str::<serde_json::Value>("{\"a\":1,}").is_err());
}
