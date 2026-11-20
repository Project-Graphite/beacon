use super::bitmap;
use crate::raster::{TextMask, TextRasterizer};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::SIZE;
use windows_sys::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateFontW, DEFAULT_CHARSET,
    DEFAULT_PITCH, DeleteDC, DeleteObject, FW_SEMIBOLD, GdiFlush, GetTextExtentPoint32W, OPAQUE,
    OUT_DEFAULT_PRECIS, SelectObject, SetBkColor, SetBkMode, SetTextColor, TextOutW,
};

const FACE: &str = "Segoe UI";
const WHITE: u32 = 0x00ff_ffff;

#[derive(Default)]
pub struct GdiText {
    cache: RefCell<HashMap<(String, u32), TextMask>>,
}

impl TextRasterizer for GdiText {
    fn rasterize(&self, text: &str, pixel_size: f32) -> TextMask {
        let key = (text.to_string(), pixel_size.round() as u32);
        self.cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| crop(draw(text, pixel_size.round() as i32)))
            .clone()
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn draw(text: &str, pixel_size: i32) -> TextMask {
    let (face, text) = (wide(FACE), wide(text));
    let length = text.len() as i32 - 1;
    unsafe {
        let dc = CreateCompatibleDC(null_mut());
        let font = CreateFontW(
            -pixel_size,
            0,
            0,
            0,
            FW_SEMIBOLD as i32,
            0,
            0,
            0,
            u32::from(DEFAULT_CHARSET),
            u32::from(OUT_DEFAULT_PRECIS),
            u32::from(CLIP_DEFAULT_PRECIS),
            u32::from(ANTIALIASED_QUALITY),
            u32::from(DEFAULT_PITCH),
            face.as_ptr(),
        );
        let previous_font = SelectObject(dc, font);
        let mut size = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(dc, text.as_ptr(), length, &mut size);
        let margin = pixel_size / 2;
        let (width, height) = (size.cx + 2 * margin, size.cy.max(1));
        let (bitmap, bits) = bitmap(dc, width, height);
        let previous_bitmap = SelectObject(dc, bitmap);
        SetTextColor(dc, WHITE);
        SetBkColor(dc, 0);
        SetBkMode(dc, OPAQUE as i32);
        TextOutW(dc, margin, 0, text.as_ptr(), length);
        GdiFlush();
        let pixels = std::slice::from_raw_parts(bits, (width * height * 4) as usize);
        let coverage = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[1])
            .collect();
        SelectObject(dc, previous_bitmap);
        SelectObject(dc, previous_font);
        DeleteObject(bitmap);
        DeleteObject(font);
        DeleteDC(dc);
        TextMask {
            width: width as usize,
            height: height as usize,
            coverage,
        }
    }
}

fn crop(mask: TextMask) -> TextMask {
    let inked = |x: usize, y: usize| mask.coverage[y * mask.width + x] > 0;
    let columns: Vec<usize> = (0..mask.width)
        .filter(|&x| (0..mask.height).any(|y| inked(x, y)))
        .collect();
    let rows: Vec<usize> = (0..mask.height)
        .filter(|&y| (0..mask.width).any(|x| inked(x, y)))
        .collect();
    let (Some(&left), Some(&right), Some(&top), Some(&bottom)) =
        (columns.first(), columns.last(), rows.first(), rows.last())
    else {
        return TextMask {
            width: 0,
            height: 0,
            coverage: Vec::new(),
        };
    };
    let width = right - left + 1;
    TextMask {
        width,
        height: bottom - top + 1,
        coverage: (top..=bottom)
            .flat_map(|y| (left..=right).map(move |x| (x, y)))
            .map(|(x, y)| mask.coverage[y * mask.width + x])
            .collect(),
    }
}
