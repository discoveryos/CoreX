// ANSI-compliant terminal stuff
// Copyright (C) 2025 kevin dan mathew
// Rust translation

#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ffi::c_int;

// -----------------------------
// External symbols from C side
// -----------------------------

extern "C" {
    fn eraseBull();
    fn updateBull();
    fn changeBg(r: u8, g: u8, b: u8);
    fn changeTextColor(r: u8, g: u8, b: u8);
    fn drawRect(x: i64, y: i64, w: i64, h: i64, r: u8, g: u8, b: u8);
    fn clearScreen();
}

// framebuffer / console globals
extern "C" {
    static mut cursorHidden: bool;

    static mut width: i64;
    static mut height: i64;

    static fb: FrameBuffer;

    static bg_color: [u8; 3];

    static TTY_CHARACTER_WIDTH: i64;
    static TTY_CHARACTER_HEIGHT: i64;
}

#[repr(C)]
pub struct FrameBuffer {
    pub width: i64,
    pub height: i64,
}

// -----------------------------
// Helpers
// -----------------------------

#[inline]
fn is_valid_number(c: i32) -> bool {
    c >= b'0' as i32 && c <= b'9' as i32
}

// -----------------------------
// ANSI color table
// -----------------------------

#[derive(Copy, Clone)]
struct ANSIColor {
    rgb: [u8; 3],
}

static ANSI_COLORS: [ANSIColor; 18] = [
    ANSIColor { rgb: [0, 0, 0] },
    ANSIColor { rgb: [170, 0, 0] },
    ANSIColor { rgb: [0, 170, 0] },
    ANSIColor { rgb: [170, 85, 0] },
    ANSIColor { rgb: [0, 0, 170] },
    ANSIColor { rgb: [170, 0, 170] },
    ANSIColor { rgb: [0, 170, 170] },
    ANSIColor { rgb: [170, 170, 170] },
    ANSIColor { rgb: [69, 69, 69] },
    ANSIColor { rgb: [69, 69, 69] },
    ANSIColor { rgb: [85, 85, 85] },
    ANSIColor { rgb: [255, 85, 85] },
    ANSIColor { rgb: [85, 255, 85] },
    ANSIColor { rgb: [255, 255, 85] },
    ANSIColor { rgb: [85, 85, 255] },
    ANSIColor { rgb: [255, 85, 255] },
    ANSIColor { rgb: [85, 255, 255] },
    ANSIColor { rgb: [255, 255, 255] },
];

// -----------------------------
// ANSI parser state
// -----------------------------

static mut asciiQuestionmark: bool = true;
static mut asciiEscaping: bool = false;
static mut asciiInside: bool = false;

static mut asciiChar1: i64 = 0;
static mut asciiFirstDone: bool = false;
static mut asciiChar2: i64 = 0;

// -----------------------------
// Core logic
// -----------------------------

/// returns true = done with escape sequence
unsafe fn asciiProcess(charnum: i32) -> bool {
    if !asciiFirstDone && charnum == b'?' as i32 {
        asciiQuestionmark = true;
        return false;
    }

    if is_valid_number(charnum) {
        let target = (charnum - b'0' as i32) as i64;
        if !asciiFirstDone {
            asciiChar1 = asciiChar1 * 10 + target;
        } else {
            asciiChar2 = asciiChar2 * 10 + target;
        }
        return false;
    }

    if charnum == b';' as i32 {
        if asciiFirstDone {
            return true;
        }
        asciiFirstDone = true;
        return false;
    }

    eraseBull();

    match charnum as u8 as char {
        'l' => {
            if asciiChar1 == 25 && asciiQuestionmark {
                cursorHidden = true;
            }
        }
        'h' => {
            if asciiChar1 == 25 && asciiQuestionmark {
                cursorHidden = false;
                updateBull();
            }
        }
        'm' => {
            if asciiChar1 == 0 && asciiChar2 == 0 {
                changeBg(0, 0, 0);
                changeTextColor(255, 255, 255);
            } else {
                if asciiChar1 >= 100 {
                    asciiChar2 = asciiChar1 - (100 - 40);
                    asciiChar1 = 1;
                }

                let extra = asciiChar1 == 1;
                let do2 = asciiChar2 >= 30;
                let controlling = if do2 { asciiChar2 } else { asciiChar1 };

                if controlling >= 30 {
                    let bg = controlling >= 40;
                    let index =
                        (controlling - 30 - if bg { 10 } else { 0 } + if extra { 10 } else { 0 })
                            as usize;

                    let color = ANSI_COLORS[index];
                    if bg {
                        changeBg(color.rgb[0], color.rgb[1], color.rgb[2]);
                    } else {
                        changeTextColor(color.rgb[0], color.rgb[1], color.rgb[2]);
                    }
                }
            }
        }
        'H' => {
            if asciiChar1 == 0 {
                asciiChar1 = 1;
            }
            if asciiChar2 == 0 {
                asciiChar2 = 1;
            }
            height = (asciiChar1 - 1) * TTY_CHARACTER_HEIGHT;
            width = (asciiChar2 - 1) * TTY_CHARACTER_WIDTH;
        }
        'C' => {
            if asciiChar1 == 0 {
                asciiChar1 = 1;
            }
            width += (asciiChar1 - 1) * TTY_CHARACTER_WIDTH;
        }
        'd' => {
            if asciiChar1 == 0 {
                asciiChar1 = 1;
            }
            height = (asciiChar1 - 1) * TTY_CHARACTER_HEIGHT;
        }
        'G' => {
            if asciiChar1 == 0 {
                asciiChar1 = 1;
            }
            width = (asciiChar1 - 1) * TTY_CHARACTER_WIDTH;
        }
        'A' => {
            if asciiChar1 == 0 {
                asciiChar1 = 1;
            }
            height -= (asciiChar1 - 2) * TTY_CHARACTER_HEIGHT;
            if height > fb.height {
                height = 0;
            }
        }
        'J' => {
            match asciiChar1 {
                0 => {
                    let restWidth = fb.width - width;
                    if restWidth > 0 {
                        drawRect(
                            width,
                            height,
                            restWidth,
                            TTY_CHARACTER_HEIGHT,
                            bg_color[0],
                            bg_color[1],
                            bg_color[2],
                        );
                    }

                    let restHeight = fb.height - (height + TTY_CHARACTER_HEIGHT);
                    if restHeight > 0 {
                        drawRect(
                            0,
                            height + TTY_CHARACTER_HEIGHT,
                            fb.width,
                            fb.height,
                            bg_color[0],
                            bg_color[1],
                            bg_color[2],
                        );
                    }
                    updateBull();
                }
                2 | 3 => {
                    clearScreen();
                }
                _ => {}
            }
        }
        _ => {}
    }

    updateBull();
    true
}

unsafe fn ansiReset() {
    asciiEscaping = false;
    asciiInside = false;
    asciiQuestionmark = false;

    asciiFirstDone = false;
    asciiChar1 = 0;
    asciiChar2 = 0;
}

/// returns true = don't echo character
#[no_mangle]
pub unsafe extern "C" fn ansiHandle(charnum: c_int) -> bool {
    if charnum == 0x1B {
        ansiReset();
        asciiEscaping = true;
        true
    } else if asciiEscaping && charnum == b'[' as i32 {
        ansiReset();
        asciiEscaping = true;
        asciiInside = true;
        true
    } else if asciiEscaping && asciiInside {
        if asciiProcess(charnum) {
            ansiReset();
        }
        true
    } else {
        false
    }
}
