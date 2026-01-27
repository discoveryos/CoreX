#![allow(dead_code)]

use core::mem::size_of;
use core::ptr;

//
// External kernel symbols
//
extern "C" {
    fn debugf(fmt: *const u8, ...);

    fn drawPixel(x: u32, y: u32, r: u32, g: u32, b: u32);

    static mut psf: *mut PSF1Font;
    static bg_color: [u32; 3];

    // VFS / memory
    fn fsKernelOpen(path: *const u8, flags: u32, mode: u32) -> *mut OpenFile;
    fn fsKernelClose(file: *mut OpenFile);
    fn fsRead(file: *mut OpenFile, buf: *mut u8, len: u32);
    fn fsGetFilesize(file: *mut OpenFile) -> u32;

    fn malloc(size: u32) -> *mut u8;
    fn free(ptr: *mut u8);
}

// ONLY included here (same rule as C)
extern "C" {
    static u_vga16_psf: [u8; 0];
}

//
// Constants
//
pub const PSF1_MAGIC: u16 = 0x0436;
pub const PSF1_MODE512: u8 = 0x01;
pub const PSF1_MODEHASTAB: u8 = 0x02;

//
// PSF structures
//
#[repr(C)]
pub struct PSF1Header {
    pub magic: u16,
    pub mode: u8,
    pub height: u8,
}

// Convenience alias: header followed by glyph data
#[repr(C)]
pub struct PSF1Font {
    pub header: PSF1Header,
    // glyphs follow immediately
}

#[repr(C)]
pub struct OpenFile {
    _private: [u8; 0],
}

//
// PSF loading
//
pub unsafe fn psf_load(buffer: *mut u8) -> bool {
    let header = buffer as *const PSF1Header;

    if (*header).magic != PSF1_MAGIC {
        debugf(
            b"[console] Invalid PSF magic! Only PSF1 is supported{0x0436} supplied{%04X}\n\0"
                .as_ptr(),
            (*header).magic as u32,
        );
        return false;
    }

    if ((*header).mode & PSF1_MODE512 == 0) &&
       ((*header).mode & PSF1_MODEHASTAB == 0)
    {
        debugf(
            b"[console] Invalid PSF mode! No unicode table found... mode{%02X}\n\0"
                .as_ptr(),
            (*header).mode as u32,
        );
        return false;
    }

    psf = buffer as *mut PSF1Font;

    debugf(
        b"[console] Initiated with font: dim(xy){%dx%d}\n\0".as_ptr(),
        8u32,
        (*header).height as u32,
    );

    true
}

pub unsafe fn psf_load_defaults() -> bool {
    psf_load(u_vga16_psf.as_ptr() as *mut u8)
}

pub unsafe fn psf_load_from_file(path: *const u8) -> bool {
    let file = fsKernelOpen(path, O_RDONLY, 0);
    if file.is_null() {
        return false;
    }

    let filesize = fsGetFilesize(file);
    let out = malloc(filesize);

    if out.is_null() {
        fsKernelClose(file);
        return false;
    }

    fsRead(file, out, filesize);
    fsKernelClose(file);

    let res = psf_load(out);
    if !res {
        free(out);
    }

    res
}

//
// Character rendering
//
pub unsafe fn psf_putc(
    c: u8,
    x: u32,
    y: u32,
    r: u32,
    g: u32,
    b: u32,
) {
    let header = &(*psf).header;

    let glyph_base = (psf as usize)
        + size_of::<PSF1Header>()
        + (c as usize * header.height as usize);

    let targ = glyph_base as *const u8;

    for i in 0..header.height {
        let row = *targ.add(i as usize);
        for j in 0..8 {
            // NOT little endian (same as C)
            if row & (1 << (8 - j)) != 0 {
                drawPixel(x + j as u32, y + i as u32, r, g, b);
            } else {
                drawPixel(
                    x + j as u32,
                    y + i as u32,
                    bg_color[0],
                    bg_color[1],
                    bg_color[2],
                );
            }
        }
    }
}
