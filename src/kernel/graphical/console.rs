// Kernel console implementation
// Copyright (C) 2020 kevin dan mathew
// Rust translation

#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ffi::c_int;
use core::sync::atomic::{AtomicBool, Ordering};

// --------------------------------
// External types & globals
// --------------------------------

#[repr(C)]
pub struct FrameBuffer {
    pub virt: *mut u8,
    pub width: u32,
    pub height: u32,
    pub pitch: usize,
}

#[repr(C)]
pub struct PSF {
    pub height: u32,
}

// spinlock
#[repr(C)]
pub struct Spinlock {
    flag: core::sync::atomic::AtomicBool,
}

extern "C" {
    // framebuffer
    static fb: FrameBuffer;

    // psf font
    static psf: *const PSF;

    fn psfLoadDefaults();
    fn psfPutC(ch: c_int, x: u32, y: u32, r: c_int, g: c_int, b: c_int);

    // ansi
    fn ansiHandle(ch: c_int) -> bool;

    // drawing
    fn drawRect(
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        r: c_int,
        g: c_int,
        b: c_int,
    );

    // spinlock
    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    // memcpy
    fn memcpy(dest: *mut u8, src: *const u8, n: usize);
}

// --------------------------------
// Globals
// --------------------------------

#[no_mangle]
pub static mut LOCK_CONSOLE: Spinlock = Spinlock {
    flag: core::sync::atomic::AtomicBool::new(false),
};

#[no_mangle]
pub static mut bg_color: [c_int; 3] = [0, 0, 0];

#[no_mangle]
pub static mut textcolor: [c_int; 3] = [255, 255, 255];

#[no_mangle]
pub static mut width: u32 = 0;

#[no_mangle]
pub static mut height: u32 = 0;

#[no_mangle]
pub static consoleDisabled: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub static mut cursorHidden: bool = false;

// --------------------------------
// Macros (translated)
// --------------------------------

#[inline(always)]
unsafe fn CHAR_HEIGHT() -> u32 {
    (*psf).height
}

const CHAR_WIDTH: u32 = 8;

// --------------------------------
// Color helpers
// --------------------------------

#[no_mangle]
pub extern "C" fn rgbToHex(r: c_int, g: c_int, b: c_int) -> u32 {
    ((r as u32 & 0xff) << 16) | ((g as u32 & 0xff) << 8) | (b as u32 & 0xff)
}

#[no_mangle]
pub extern "C" fn rgbaToHex(r: c_int, g: c_int, b: c_int, a: c_int) -> u32 {
    ((r as u32 & 0xff) << 24)
        | ((g as u32 & 0xff) << 16)
        | ((b as u32 & 0xff) << 8)
        | (a as u32 & 0xff)
}

// --------------------------------
// Console core
// --------------------------------

#[no_mangle]
pub unsafe extern "C" fn initiateConsole() {
    width = 0;
    height = 0;
    psfLoadDefaults();
}

#[no_mangle]
pub unsafe extern "C" fn scrollConsole(check: bool) -> bool {
    if check && !(height >= fb.height - CHAR_HEIGHT()) {
        return false;
    }

    let char_h = CHAR_HEIGHT() as usize;

    let mut y = char_h;
    while y < fb.height as usize {
        let dest = fb.virt.add((y - char_h) * fb.pitch);
        let src = fb.virt.add(y * fb.pitch);
        memcpy(dest, src, fb.width as usize * 4);
        y += 1;
    }

    drawRect(
        0,
        fb.height - CHAR_HEIGHT(),
        fb.width,
        CHAR_HEIGHT(),
        bg_color[0],
        bg_color[1],
        bg_color[2],
    );

    height -= CHAR_HEIGHT();
    true
}

#[no_mangle]
pub unsafe extern "C" fn eraseBull() {
    if cursorHidden {
        return;
    }

    drawRect(
        width,
        height,
        CHAR_WIDTH,
        CHAR_HEIGHT(),
        bg_color[0],
        bg_color[1],
        bg_color[2],
    );
}

#[no_mangle]
pub unsafe extern "C" fn updateBull() {
    if cursorHidden {
        return;
    }

    if width >= fb.width {
        let needed = scrollConsole(true);
        if !needed {
            height += CHAR_HEIGHT();
        }
        width = 0;
    }

    drawRect(
        width,
        height,
        CHAR_WIDTH,
        CHAR_HEIGHT(),
        textcolor[0],
        textcolor[1],
        textcolor[2],
    );
}

#[no_mangle]
pub unsafe extern "C" fn clearScreen() {
    width = 0;
    height = 0;
    drawRect(
        0,
        0,
        fb.width,
        fb.height,
        bg_color[0],
        bg_color[1],
        bg_color[2],
    );
    updateBull();
}

// --------------------------------
// Color setters
// --------------------------------

#[no_mangle]
pub extern "C" fn changeTextColor(r: c_int, g: c_int, b: c_int) {
    unsafe {
        textcolor[0] = r;
        textcolor[1] = g;
        textcolor[2] = b;
    }
}

#[no_mangle]
pub extern "C" fn changeBg(r: c_int, g: c_int, b: c_int) {
    unsafe {
        bg_color[0] = r;
        bg_color[1] = g;
        bg_color[2] = b;
    }
}

#[no_mangle]
pub extern "C" fn changeColor(color: *const c_int) {
    unsafe {
        textcolor[0] = *color;
        textcolor[1] = *color.add(1);
        textcolor[2] = *color.add(2);
    }
}

// --------------------------------
// Cursor getters/setters
// --------------------------------

#[no_mangle]
pub extern "C" fn getConsoleX() -> u32 {
    unsafe { width }
}

#[no_mangle]
pub extern "C" fn getConsoleY() -> u32 {
    unsafe { height }
}

#[no_mangle]
pub extern "C" fn setConsoleX(x: u32) {
    unsafe {
        eraseBull();
        width = x;
        updateBull();
    }
}

#[no_mangle]
pub extern "C" fn setConsoleY(y: u32) {
    unsafe {
        eraseBull();
        height = y;
        updateBull();
    }
}

// --------------------------------
// Character output
// --------------------------------

#[no_mangle]
pub unsafe extern "C" fn drawCharacter(charnum: c_int) {
    if charnum == 0 || consoleDisabled.load(Ordering::Relaxed) {
        return;
    }

    if ansiHandle(charnum) {
        return;
    }

    if width > fb.width - CHAR_WIDTH {
        width = 0;
        height += CHAR_HEIGHT();
    }

    scrollConsole(true);

    match charnum {
        -1 => {
            drawRect(
                width,
                height,
                CHAR_WIDTH,
                CHAR_HEIGHT(),
                bg_color[0],
                bg_color[1],
                bg_color[2],
            );
            width += CHAR_WIDTH;
        }
        b'\n' as c_int => {
            eraseBull();
            width = 0;
            height += CHAR_HEIGHT();
        }
        0x0d => {
            eraseBull();
            width = 0;
        }
        0x0f => {}
        b'\x08' as c_int => {
            eraseBull();
            width -= CHAR_WIDTH;
            drawRect(
                width,
                height,
                CHAR_WIDTH,
                CHAR_HEIGHT(),
                bg_color[0],
                bg_color[1],
                bg_color[2],
            );
        }
        b'\t' as c_int => {
            for _ in 0..4 {
                drawCharacter(b' ' as c_int);
            }
        }
        _ => {
            eraseBull();
            psfPutC(
                charnum,
                width,
                height,
                textcolor[0],
                textcolor[1],
                textcolor[2],
            );
            width += CHAR_WIDTH;
        }
    }

    updateBull();
}

// --------------------------------
// printf glue
// --------------------------------

#[no_mangle]
pub extern "C" fn printfch(character: c_int) {
    unsafe {
        spinlockAcquire(&mut LOCK_CONSOLE);
        drawCharacter(character);
        spinlockRelease(&mut LOCK_CONSOLE);
    }
}

// printf.c uses this
#[no_mangle]
pub extern "C" fn putchar_(c: c_int) {
    printfch(c);
}
