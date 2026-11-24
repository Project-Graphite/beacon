use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::Instant;

const ERROR_PIPE_BUSY: i32 = 231;
const ERROR_ACCESS_DENIED: u32 = 5;
const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_TIMEOUT: u32 = 258;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

#[repr(C)]
struct StartupInfo {
    size: u32,
    reserved: *const u16,
    desktop: *const u16,
    title: *const u16,
    placement: [u32; 8],
    show_window: u16,
    reserved_size: u16,
    reserved_bytes: *const u8,
    std_handles: [*mut c_void; 3],
}

#[repr(C)]
struct ProcessInformation {
    process: *mut c_void,
    thread: *mut c_void,
    process_id: u32,
    thread_id: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitNamedPipeW(name: *const u16, timeout_ms: u32) -> i32;
    fn CreateProcessW(
        application: *const u16,
        command_line: *mut u16,
        process_attributes: *const c_void,
        thread_attributes: *const c_void,
        inherit_handles: i32,
        creation_flags: u32,
        environment: *const c_void,
        current_directory: *const u16,
        startup_info: *const StartupInfo,
        process_information: *mut ProcessInformation,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn OpenProcess(access: u32, inherit_handle: i32, pid: u32) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, timeout_ms: u32) -> u32;
    fn GetLastError() -> u32;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub fn open_pipe(name: &str, deadline: Instant) -> io::Result<File> {
    let wide_name = wide(name);
    loop {
        match OpenOptions::new().read(true).write(true).open(name) {
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                let remaining_ms = deadline
                    .checked_duration_since(Instant::now())
                    .map_or(0, |remaining| remaining.as_millis());
                if remaining_ms == 0 {
                    return Err(error);
                }
                unsafe { WaitNamedPipeW(wide_name.as_ptr(), remaining_ms as u32) };
            }
            opened => return opened,
        }
    }
}

pub fn process_alive(pid: u32) -> bool {
    let process = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
    if process.is_null() {
        return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
    }
    let running = unsafe { WaitForSingleObject(process, 0) } == WAIT_TIMEOUT;
    unsafe { CloseHandle(process) };
    running
}

pub fn start_detached(executable: &Path, argument: &str) {
    let application = wide(&executable.to_string_lossy());
    let directory = executable
        .parent()
        .map(|parent| wide(&parent.to_string_lossy()));
    let startup = StartupInfo {
        size: size_of::<StartupInfo>() as u32,
        reserved: null(),
        desktop: null(),
        title: null(),
        placement: [0; 8],
        show_window: 0,
        reserved_size: 0,
        reserved_bytes: null(),
        std_handles: [null_mut(); 3],
    };
    for flags in [
        DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB,
        DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
    ] {
        let mut command_line = wide(&format!("\"{}\" {argument}", executable.display()));
        let mut started = ProcessInformation {
            process: null_mut(),
            thread: null_mut(),
            process_id: 0,
            thread_id: 0,
        };
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                0,
                flags,
                null(),
                directory
                    .as_ref()
                    .map_or(null(), |directory| directory.as_ptr()),
                &startup,
                &mut started,
            )
        };
        if created != 0 {
            unsafe {
                CloseHandle(started.thread);
                CloseHandle(started.process);
            }
            return;
        }
    }
}
