#[derive(Debug, Copy, Clone)]
pub struct Flags {
    bits: u32,
}

#[allow(dead_code)]
impl Flags {
    pub fn new() -> Flags {
        Flags { bits: 0 }
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