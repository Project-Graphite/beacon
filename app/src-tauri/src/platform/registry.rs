use std::io;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_DWORD, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn checked(status: WIN32_ERROR) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

pub fn write_string(key: &str, name: &str, value: &str) -> io::Result<()> {
    let data = wide(value);
    checked(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            wide(key).as_ptr(),
            wide(name).as_ptr(),
            REG_SZ,
            data.as_ptr().cast(),
            (data.len() * size_of::<u16>()) as u32,
        )
    })
}

pub fn read_dword(key: &str, name: &str) -> Option<u32> {
    let (mut value, mut size) = (0u32, size_of::<u32>() as u32);
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(key).as_ptr(),
            wide(name).as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

pub fn delete_value(key: &str, name: &str) -> io::Result<()> {
    match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(key).as_ptr(), wide(name).as_ptr()) }
    {
        ERROR_FILE_NOT_FOUND => Ok(()),
        status => checked(status),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use windows_sys::Win32::System::Registry::{RRF_RT_REG_SZ, RegDeleteKeyW};

    pub fn delete_key(key: &str) -> io::Result<()> {
        checked(unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, wide(key).as_ptr()) })
    }

    pub fn has_value(key: &str, name: &str) -> io::Result<bool> {
        let mut size = 0;
        match unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                wide(key).as_ptr(),
                wide(name).as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                null_mut(),
                &mut size,
            )
        } {
            ERROR_FILE_NOT_FOUND => Ok(false),
            status => checked(status).map(|()| true),
        }
    }
}
