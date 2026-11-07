use crate::{blocks, inline, render};
use std::ffi::{c_char, CString};
use std::{ptr, slice, str};

unsafe fn text<'a>(pointer: *const u8, length: usize) -> &'a str {
    str::from_utf8(slice::from_raw_parts(pointer, length)).unwrap_or("")
}

#[no_mangle]
pub unsafe extern "C" fn wc_blocks_json(pointer: *const u8, length: usize) -> *mut c_char {
    let json = blocks::blocks_json(text(pointer, length));
    CString::new(json).map_or(ptr::null_mut(), CString::into_raw)
}

#[no_mangle]
pub unsafe extern "C" fn wc_free_string(pointer: *mut c_char) {
    if !pointer.is_null() {
        drop(CString::from_raw(pointer));
    }
}

#[repr(C)]
pub struct WcSpan {
    pub kind: u32,
    pub start: u32,
    pub end: u32,
}

#[no_mangle]
pub unsafe extern "C" fn wc_inline_spans(
    pointer: *const u8,
    length: usize,
    output: *mut WcSpan,
    capacity: usize,
) -> usize {
    let spans = inline::inline_spans(text(pointer, length));
    let count = spans.len().min(capacity);
    for (index, span) in spans.iter().take(count).enumerate() {
        output.add(index).write(WcSpan { kind: span.kind, start: span.start, end: span.end });
    }
    count
}

#[repr(C)]
pub struct WcImage {
    pub bytes: *mut u8,
    pub length: usize,
    pub width: f32,
    pub height: f32,
}

#[no_mangle]
pub unsafe extern "C" fn wc_render_block(
    kind_pointer: *const u8,
    kind_length: usize,
    pointer: *const u8,
    length: usize,
) -> WcImage {
    let empty = WcImage { bytes: ptr::null_mut(), length: 0, width: 0.0, height: 0.0 };
    let kind = text(kind_pointer, kind_length);
    match render::render_block(kind, text(pointer, length)) {
        Ok(image) => {
            let boxed = image.png.into_boxed_slice();
            let length = boxed.len();
            WcImage {
                bytes: Box::into_raw(boxed) as *mut u8,
                length,
                width: image.width,
                height: image.height,
            }
        }
        Err(_) => empty,
    }
}

#[no_mangle]
pub unsafe extern "C" fn wc_free_image(image: WcImage) {
    if !image.bytes.is_null() {
        drop(Box::from_raw(slice::from_raw_parts_mut(image.bytes, image.length)));
    }
}
