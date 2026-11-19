use std::fs::{File, OpenOptions};
use std::time::Instant;

const ERROR_PIPE_BUSY: i32 = 231;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitNamedPipeW(name: *const u16, timeout_ms: u32) -> i32;
}

pub fn open_pipe(name: &str, deadline: Instant) -> Option<File> {
    let wide_name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    loop {
        match OpenOptions::new().read(true).write(true).open(name) {
            Ok(pipe) => return Some(pipe),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                let remaining_ms = deadline.checked_duration_since(Instant::now())?.as_millis();
                if remaining_ms == 0 {
                    return None;
                }
                unsafe { WaitNamedPipeW(wide_name.as_ptr(), remaining_ms as u32) };
            }
            Err(_) => return None,
        }
    }
}
