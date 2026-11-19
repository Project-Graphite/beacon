use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_PIPE_BUSY, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, LocalFree, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateMutexW, GetCurrentProcess, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, RegisterWaitForSingleObject,
    UnregisterWaitEx, WT_EXECUTEONLYONCE, WaitForSingleObject,
};

pub const PIPE_PREFIX: &str = r"\\.\pipe\beacon-";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn check(succeeded: i32) -> io::Result<()> {
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            N as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err(io::Error::other(format!(
            "BCryptGenRandom failed with status {status:#x}"
        )));
    }
    Ok(bytes)
}

pub struct InstanceLock(HANDLE);

unsafe impl Send for InstanceLock {}

impl InstanceLock {
    pub fn acquire(home: &Path) -> io::Result<InstanceLock> {
        let key = std::fs::canonicalize(home)?
            .to_string_lossy()
            .to_lowercase();
        let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        });
        let name = wide(&format!(r"Local\beacon-core-{hash:016x}"));
        let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(handle) };
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("a Beacon core is already running for {}", home.display()),
            ));
        }
        Ok(InstanceLock(handle))
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub struct PipeSecurity {
    attributes: SECURITY_ATTRIBUTES,
}

unsafe impl Send for PipeSecurity {}

impl PipeSecurity {
    pub fn current_user_only() -> io::Result<PipeSecurity> {
        let sddl = wide(&format!("D:P(A;;GA;;;{})", current_user_sid()?));
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        check(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        })?;
        Ok(PipeSecurity {
            attributes: SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        })
    }

    pub fn attributes(&self) -> *mut c_void {
        (&self.attributes as *const SECURITY_ATTRIBUTES)
            .cast_mut()
            .cast()
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        unsafe { LocalFree(self.attributes.lpSecurityDescriptor) };
    }
}

fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = null_mut();
    check(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) })?;
    let mut needed = 0u32;
    unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed) };
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    let filled = check(unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    });
    unsafe { CloseHandle(token) };
    filled?;
    let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    let mut text = null_mut();
    check(unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) })?;
    let length = (0..)
        .take_while(|&index| unsafe { *text.add(index) } != 0)
        .count();
    let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    unsafe { LocalFree(text.cast()) };
    Ok(sid)
}

pub fn open_pipe(name: &str, timeout: Duration) -> io::Result<File> {
    let deadline = Instant::now() + timeout;
    let wide_name = wide(name);
    loop {
        match OpenOptions::new().read(true).write(true).open(name) {
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                let remaining_ms = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u32;
                if remaining_ms == 0 {
                    return Err(error);
                }
                unsafe { WaitNamedPipeW(wide_name.as_ptr(), remaining_ms) };
            }
            opened => return opened,
        }
    }
}

pub fn process_alive(pid: u32) -> bool {
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if process.is_null() {
        return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
    }
    let running = unsafe { WaitForSingleObject(process, 0) } == WAIT_TIMEOUT;
    unsafe { CloseHandle(process) };
    running
}

type ExitCallback = Box<dyn Fn() + Send + Sync>;

pub struct ProcessWatch {
    wait: HANDLE,
    process: HANDLE,
    callback: *mut ExitCallback,
}

impl ProcessWatch {
    pub fn start(pid: u32, on_exit: ExitCallback) -> io::Result<Option<ProcessWatch>> {
        let process = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        if process.is_null() {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
                Err(error)
            } else {
                Ok(None)
            };
        }
        let callback = Box::into_raw(Box::new(on_exit));
        let mut wait: HANDLE = null_mut();
        let registered = check(unsafe {
            RegisterWaitForSingleObject(
                &mut wait,
                process,
                Some(process_exited),
                callback as *const c_void,
                u32::MAX,
                WT_EXECUTEONLYONCE,
            )
        });
        if let Err(error) = registered {
            unsafe {
                CloseHandle(process);
                drop(Box::from_raw(callback));
            }
            return Err(error);
        }
        Ok(Some(ProcessWatch {
            wait,
            process,
            callback,
        }))
    }
}

unsafe extern "system" fn process_exited(context: *mut c_void, _timed_out: bool) {
    let callback = unsafe { &*(context as *const ExitCallback) };
    callback();
}

impl Drop for ProcessWatch {
    fn drop(&mut self) {
        unsafe {
            UnregisterWaitEx(self.wait, INVALID_HANDLE_VALUE);
            CloseHandle(self.process);
            drop(Box::from_raw(self.callback));
        }
    }
}

pub fn hide_console_window(command: &mut tokio::process::Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}
