use serde::{Deserialize, Serialize};

/// 20.12 fixed point, as the DS hardware and the game use it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Fx32(i32);

impl Fx32 {
    pub const SCALE: i32 = 4096;

    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn from_integer(value: i32) -> Self {
        Self(value.wrapping_mul(Self::SCALE))
    }

    pub const fn raw(self) -> i32 {
        self.0
    }

    pub fn value(self) -> f64 {
        f64::from(self.0) / f64::from(Self::SCALE)
    }
}
