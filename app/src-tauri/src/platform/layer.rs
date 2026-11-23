use super::bitmap;
use crate::geometry::Rect;
use crate::raster::Canvas;
use std::io;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, CreateCompatibleDC, DeleteObject, HBITMAP, HDC,
    SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, HWND_TOPMOST, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u8,
    width: i32,
    height: i32,
}

pub struct Layer {
    pub hwnd: HWND,
    surface: Surface,
    frame: Vec<u8>,
    rect: Rect,
    alpha: u8,
    shown: bool,
}

impl Surface {
    fn resize(&mut self, width: i32, height: i32) -> bool {
        let (created, bits) = bitmap(self.dc, width, height);
        if created.is_null() || bits.is_null() {
            log_error!(
                "cannot create a layer bitmap: {}",
                io::Error::last_os_error()
            );
            return false;
        }
        unsafe {
            SelectObject(self.dc, created);
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
        }
        (self.bitmap, self.bits, self.width, self.height) = (created, bits, width, height);
        true
    }

    fn pixels(&mut self) -> &mut [u8] {
        if self.bits.is_null() {
            return &mut [];
        }
        unsafe {
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        }
    }
}

impl Layer {
    pub fn create(class: &[u16]) -> io::Result<Layer> {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Layer {
            hwnd,
            surface: Surface {
                dc: unsafe { CreateCompatibleDC(null_mut()) },
                bitmap: null_mut(),
                bits: null_mut(),
                width: 0,
                height: 0,
            },
            frame: Vec::new(),
            rect: Rect::default(),
            alpha: 0,
            shown: false,
        })
    }

    pub fn update(&mut self, rect: Rect, canvas: &Canvas, alpha: u8) {
        canvas.write_premultiplied_bgra(&mut self.frame);
        let resized = (rect.width(), rect.height()) != (self.surface.width, self.surface.height);
        if !resized
            && rect == self.rect
            && alpha == self.alpha
            && *self.surface.pixels() == self.frame[..]
        {
            return;
        }
        if resized && !self.surface.resize(rect.width(), rect.height()) {
            return;
        }
        self.surface.pixels().copy_from_slice(&self.frame);
        unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                &POINT {
                    x: rect.left,
                    y: rect.top,
                },
                &SIZE {
                    cx: rect.width(),
                    cy: rect.height(),
                },
                self.surface.dc,
                &POINT { x: 0, y: 0 },
                0,
                &BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: alpha,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                },
                ULW_ALPHA,
            );
        }
        self.rect = rect;
        self.alpha = alpha;
    }

    pub fn show(&mut self, shown: bool) {
        if shown != self.shown {
            self.shown = shown;
            unsafe {
                if shown {
                    SetWindowPos(
                        self.hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                } else {
                    ShowWindow(self.hwnd, SW_HIDE);
                }
            }
        }
    }
}
