use chrono::{Local, Timelike};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use terminal_color_builder::OutputFormatter as tcb;

pub trait LogOutput {
    fn log(&mut self, log: &str);
    fn success(&mut self, success: &str);
    fn error(&mut self, error: &str);
    fn warning(&mut self, warning: &str);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoutLogger {}

impl LogOutput for CoutLogger {
    fn log(&mut self, log: &str) {
        let time = Local::now();
        println!(
            "[{:02}:{:02}:{:02}] {}",
            time.hour(),
            time.minute(),
            time.second(),
            log
        );
    }

    fn success(&mut self, success: &str) {
        let time = Local::now();
        println!(
            "[{:02}:{:02}:{:02}] {}",
            time.hour(),
            time.minute(),
            time.second(),
            tcb::new().fg().hex("00af00").text_str(success).print()
        );
    }

    fn error(&mut self, error: &str) {
        let time = Local::now();
        eprintln!(
            "[{:02}:{:02}:{:02}] {}",
            time.hour(),
            time.minute(),
            time.second(),
            tcb::new().fg().hex("d70000").text_str(error).print()
        );
    }

    fn warning(&mut self, warning: &str) {
        let time = Local::now();
        eprintln!(
            "[{:02}:{:02}:{:02}] {}",
            time.hour(),
            time.minute(),
            time.second(),
            tcb::new().fg().hex("d75f00").text_str(warning).print()
        );
    }
}

pub struct FileLogger {
    file: PathBuf,
    buffer: String,
}

impl Drop for FileLogger {
    fn drop(&mut self) {
        if let Ok(mut file) = OpenOptions::new().read(false).append(true).open(&self.file) {
            if file.write_all(self.buffer.as_bytes()).is_err() {
                eprintln!("Could not write log output to {}", self.file.display());
            }
        }
    }
}

impl FileLogger {
    pub fn new<T: AsRef<Path>>(path: T) -> Self {
        let file = path.as_ref().to_path_buf();
        Self {
            file,
            buffer: String::with_capacity(16384),
        }
    }

    pub fn write(&mut self, string: &str) {
        let time = Local::now();
        let string = format!(
            "[{:02}:{:02}:{:02}] {}\n",
            time.hour(),
            time.minute(),
            time.second(),
            string
        );

        self.buffer += string.as_str();
    }
}

impl LogOutput for FileLogger {
    fn log(&mut self, log: &str) {
        self.write(log);
    }

    fn success(&mut self, success: &str) {
        self.write(success);
    }

    fn error(&mut self, error: &str) {
        self.write(error);
    }

    fn warning(&mut self, warning: &str) {
        self.write(warning);
    }
}

pub struct BufferLogger {
    buffer: Vec<String>,
    capacity: usize,
    ptr: usize,
}

impl Default for BufferLogger {
    fn default() -> Self {
        Self::with_capacity(1000)
    }
}

impl BufferLogger {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            ptr: 0,
        }
    }

    pub fn push(&mut self, log: String) {
        if self.ptr >= self.capacity {
            self.ptr = 1;
            self.buffer[0] = log;
        } else {
            let index = self.ptr;
            self.ptr += 1;
            self.buffer[index] = log;
        }
    }

    pub fn print(&self) {
        for i in 0..self.ptr {
            println!("{}", self.buffer[i]);
        }

        for i in self.ptr..(self.capacity.min(self.buffer.len())) {
            println!("{}", self.buffer[i]);
        }
    }
}

impl LogOutput for BufferLogger {
    fn log(&mut self, log: &str) {
        self.push(log.to_string());
    }

    fn success(&mut self, success: &str) {
        self.push(success.to_string());
    }

    fn error(&mut self, error: &str) {
        self.push(error.to_string());
    }

    fn warning(&mut self, warning: &str) {
        self.push(warning.to_string());
    }
}

static mut LOGGER: Option<Mutex<Box<dyn LogOutput>>> = None;

pub fn set_logger<T: 'static + LogOutput>(output: T) {
    unsafe {
        LOGGER = Some(Mutex::new(Box::new(output)));
    }
}

pub fn log<T: AsRef<str>>(message: T) {
    let mut l = unsafe {
        if let Some(l) = LOGGER.as_ref() {
            if let Ok(l) = l.lock() {
                l
            } else {
                return;
            }
        } else {
            return;
        }
    };

    l.log(message.as_ref())
}

pub fn success<T: AsRef<str>>(message: T) {
    let mut l = unsafe {
        if let Some(l) = LOGGER.as_ref() {
            if let Ok(l) = l.lock() {
                l
            } else {
                return;
            }
        } else {
            return;
        }
    };

    l.success(message.as_ref())
}

pub fn error<T: AsRef<str>>(message: T) {
    let mut l = unsafe {
        if let Some(l) = LOGGER.as_ref() {
            if let Ok(l) = l.lock() {
                l
            } else {
                return;
            }
        } else {
            return;
        }
    };

    l.error(message.as_ref())
}

pub fn warning<T: AsRef<str>>(message: T) {
    let mut l = unsafe {
        if let Some(l) = LOGGER.as_ref() {
            if let Ok(l) = l.lock() {
                l
            } else {
                return;
            }
        } else {
            return;
        }
    };

    l.warning(message.as_ref())
}
