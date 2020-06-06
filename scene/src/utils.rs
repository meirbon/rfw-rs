use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Flags {
    bits: u32,
}

#[allow(dead_code)]
impl Flags {
    pub fn new() -> Flags {
        Self::default()
    }

    pub fn set_flag<T: Into<u8>>(&mut self, flag: T) {
        self.bits |= flag.into() as u8 as u32;
    }

    pub fn unset_flag<T: Into<u8>>(&mut self, flag: T) {
        self.bits &= (!(flag.into() as u8)) as u32;
    }

    pub fn has_flag<T: Into<u8>>(&self, flag: T) -> bool {
        self.bits & (flag.into() as u8) as u32 > 0
    }
}

impl Default for Flags {
    fn default() -> Self {
        Self { bits: 0 }
    }
}
