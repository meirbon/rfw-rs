use std::time::{Duration, Instant};

pub struct Timer {
    moment: Instant
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            moment: Instant::now()
        }
    }

    pub fn reset(&mut self) {
        self.moment = Instant::now();
    }

    pub fn elapsed(&self) -> Duration {
        self.moment.elapsed()
    }

    pub fn elapsed_in_millis(&self) -> f32 {
        let elapsed = self.elapsed();
        let secs = elapsed.as_secs() as u32;
        let millis = elapsed.subsec_millis();
        (secs * 1_000_000 + millis) as f32
    }
}


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