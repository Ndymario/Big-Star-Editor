//! BMG detection. The codec arrives with phase 8.

pub fn is_bmg(bytes: &[u8], path: &str) -> bool {
    if bytes.starts_with(b"MESGbmg1") {
        return true;
    }
    bytes.is_empty() && path.to_ascii_lowercase().ends_with(".bmg")
}
