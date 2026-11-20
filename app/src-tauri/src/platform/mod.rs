pub mod pill;
mod text;

pub use text::GdiText;

use crate::geometry::{Monitor, Rect};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, EnumDisplayMonitors,
    GetMonitorInfoW, HBITMAP, HDC, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
    MonitorFromPoint,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, MONITORINFOF_PRIMARY, SM_CXSMICON, SPI_GETCLIENTAREAANIMATION,
    SetForegroundWindow, SystemParametersInfoW,
};
use windows_sys::core::BOOL;

const BASE_DPI: f64 = 96.0;

fn rect(rect: RECT) -> Rect {
    Rect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

fn monitor_dpi(monitor: HMONITOR) -> u32 {
    let (mut dpi_x, mut dpi_y) = (BASE_DPI as u32, BASE_DPI as u32);
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    dpi_x
}

fn monitor_info(monitor: HMONITOR) -> Option<MONITORINFO> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then_some(info)
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(data as *mut Vec<Monitor>) };
    if let Some(info) = monitor_info(monitor) {
        monitors.push(Monitor {
            bounds: rect(info.rcMonitor),
            work: rect(info.rcWork),
            scale: f64::from(monitor_dpi(monitor)) / BASE_DPI,
            primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    1
}

pub fn monitors() -> Vec<Monitor> {
    let mut monitors = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(collect_monitor),
            &mut monitors as *mut Vec<Monitor> as LPARAM,
        )
    };
    monitors
}

pub fn cursor_position() -> (i32, i32) {
    let mut point = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut point) };
    (point.x, point.y)
}

pub fn reduced_motion() -> bool {
    let mut animation: BOOL = 1;
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut animation as *mut BOOL as *mut c_void,
            0,
        )
    };
    animation == 0
}

pub fn tray_icon_size() -> usize {
    let primary = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, monitor_dpi(primary)) }.max(16) as usize
}

pub fn focus_window(hwnd: *mut c_void) {
    unsafe { SetForegroundWindow(hwnd) };
}

fn bitmap(dc: HDC, width: i32, height: i32) -> (HBITMAP, *mut u8) {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..unsafe { std::mem::zeroed() }
        },
        ..unsafe { std::mem::zeroed() }
    };
    let mut bits = null_mut();
    let bitmap = unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    (bitmap, bits.cast())
}
