pub mod pill;
mod text;

pub use text::GdiText;

use crate::geometry::{Monitor, Rect};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows_sys::Win32::Foundation::{LPARAM, LUID, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, EnumDisplayMonitors,
    GetMonitorInfoW, HBITMAP, HDC, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MONITORINFOEXW,
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
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..unsafe { std::mem::zeroed() }
        },
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe {
        GetMonitorInfoW(
            monitor,
            &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
        )
    } != 0
    {
        let device = text_of(&info.szDevice);
        monitors.push(Monitor {
            id: device.clone(),
            name: device,
            bounds: rect(info.monitorInfo.rcMonitor),
            work: rect(info.monitorInfo.rcWork),
            scale: f64::from(monitor_dpi(monitor)) / BASE_DPI,
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    1
}

fn text_of(buffer: &[u16]) -> String {
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..length])
}

fn device_info_header<T>(kind: i32, adapter: LUID, id: u32) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size: size_of::<T>() as u32,
        adapterId: adapter,
        id,
    }
}

fn display_targets() -> Vec<(String, String, String)> {
    let (mut path_count, mut mode_count) = (0, 0);
    if unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    } != 0
    {
        return Vec::new();
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    if unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            null_mut(),
        )
    } != 0
    {
        return Vec::new();
    }
    paths.truncate(path_count as usize);
    paths
        .iter()
        .filter_map(|path| {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: device_info_header::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(
                    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    path.sourceInfo.adapterId,
                    path.sourceInfo.id,
                ),
                ..Default::default()
            };
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: device_info_header::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(
                    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    path.targetInfo.adapterId,
                    path.targetInfo.id,
                ),
                ..Default::default()
            };
            let found = unsafe {
                DisplayConfigGetDeviceInfo(&mut source.header) == 0
                    && DisplayConfigGetDeviceInfo(&mut target.header) == 0
            };
            found.then(|| {
                (
                    text_of(&source.viewGdiDeviceName),
                    text_of(&target.monitorFriendlyDeviceName),
                    text_of(&target.monitorDevicePath),
                )
            })
        })
        .collect()
}

pub fn monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(collect_monitor),
            &mut monitors as *mut Vec<Monitor> as LPARAM,
        )
    };
    let targets = display_targets();
    monitors
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let (id, name) = targets
                .iter()
                .find(|(device, _, _)| *device == monitor.id)
                .map_or(
                    (monitor.id.clone(), monitor.name.clone()),
                    |(_, name, path)| (path.clone(), name.clone()),
                );
            Monitor {
                id,
                name: if name.is_empty() {
                    format!("Display {}", index + 1)
                } else {
                    name
                },
                ..monitor
            }
        })
        .collect()
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
