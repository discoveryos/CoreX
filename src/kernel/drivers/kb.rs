#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU32, Ordering};

/// ---------------------------
/// PORT I/O
/// ---------------------------
unsafe fn outportb(port: u16, value: u8) {
    core::arch::asm!("outb %al, %dx", in("dx") port, in("al") value);
}

unsafe fn inportb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("inb %dx, %al", out("al") val, in("dx") port);
    val
}

/// ---------------------------
/// Constants & Scancodes
/// ---------------------------
const SCANCODE_ENTER: u8 = 0x1C;
const SCANCODE_BACK: u8 = 0x0E;
const SCANCODE_SHIFT: u8 = 0x2A;
const SCANCODE_CAPS: u8 = 0x3A;

const CHARACTER_ENTER: char = '\n';
const CHARACTER_BACK: char = '\x08';

static CHARACTER_TABLE: [u8; 128] = [
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
    b'-', b'=', 0, 9, b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i',
    b'o', b'p', b'[', b']', 0, 0, b'a', b's', b'd', b'f', b'g', b'h',
    b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\', b'z', b'x', b'c',
    b'v', b'b', b'n', b'm', b',', b'.', b'/', 0, b'*', 0, b' ', 0, 0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0x1B,0,0,0,0,0,0,0,0,0,0,0x0E,
    0x1C,0,0,0,0,0,0,0,'/',0,0,0,0,0,0,0,0,0,0,0,0,0,0x1E,0x1F,0x20,0x21,
    0x22,0x23,0x24,0x25,0x26,0x27,0x28,0,0,0,0,0,0,0,0x2C
];

static SHIFTED_CHARACTER_TABLE: [u8; 128] = [
    0,27,b'!',b'@',b'#',b'$',b'%',b'^',b'&',b'*',b'(',b')',
    b'_',b'+',0,9,b'Q',b'W',b'E',b'R',b'T',b'Y',b'U',b'I',
    b'O',b'P',b'{',b'}',0,0,b'A',b'S',b'D',b'F',b'G',b'H',
    b'J',b'K',b'L',b':',b'"',b'~',0,b'|',b'Z',b'X',b'C',b'V',
    b'B',b'N',b'M',b'<',b'>',b'?',0,b'*',0,b' ',0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0x1B,0,0,0,0,0,0,0,0,0,0,0x0E,0x1C,0,0,0,0,0,0,0, b'?',0,0,0,0,0,0,0,0,0,0,0,0,0x1E,0x1F,0x20,0x21,0x22,0x23,0x24,0x25,0x26,0x27,0x28,0,0,0,0,0,0,0,0x2C
];

/// ---------------------------
/// Keyboard State
/// ---------------------------
static mut SHIFTED: bool = false;
static mut CAPSLOCKED: bool = false;
static mut KB_BUFF: *mut u8 = core::ptr::null_mut();
static mut KB_CURR: usize = 0;
static mut KB_MAX: usize = 0;
static mut KB_TASK_ID: u32 = 0;

/// Bitmap for key pressed/released tracking
const EVDEV_INTERNAL_SIZE: usize = 16;
static mut EVDEV_INTERNAL: [u8; EVDEV_INTERNAL_SIZE] = [0; EVDEV_INTERNAL_SIZE];

/// ---------------------------
/// Helpers for bitmap
/// ---------------------------
fn bitmap_get(map: &[u8], index: usize) -> bool {
    let byte = map[index / 8];
    byte & (1 << (index % 8)) != 0
}

fn bitmap_set(map: &mut [u8], index: usize, value: bool) {
    let byte = &mut map[index / 8];
    if value {
        *byte |= 1 << (index % 8);
    } else {
        *byte &= !(1 << (index % 8));
    }
}

/// ---------------------------
/// Low-level keyboard I/O
/// ---------------------------
unsafe fn kb_read() -> u8 {
    while inportb(0x64) & 1 == 0 {}
    inportb(0x60)
}

unsafe fn kb_write(port: u16, value: u8) {
    while inportb(0x64) & 2 != 0 {}
    outportb(port, value);
}

/// ---------------------------
/// Handle key press/release
/// ---------------------------
pub unsafe fn handle_kb_event() -> u8 {
    let scan_code = kb_read();

    // Shift key
    if scan_code == SCANCODE_SHIFT && (scan_code & 0x80) == 0 {
        SHIFTED = true;
        return 0;
    } else if scan_code == SCANCODE_SHIFT && (scan_code & 0x80) != 0 {
        SHIFTED = false;
        return 0;
    }

    if (scan_code as usize) < CHARACTER_TABLE.len() && (scan_code & 0x80) == 0 {
        let character = if SHIFTED || CAPSLOCKED {
            SHIFTED_CHARACTER_TABLE[scan_code as usize]
        } else {
            CHARACTER_TABLE[scan_code as usize]
        };

        if character != 0 {
            return character;
        }

        match scan_code {
            SCANCODE_ENTER => return CHARACTER_ENTER as u8,
            SCANCODE_BACK => return CHARACTER_BACK as u8,
            SCANCODE_CAPS => {
                CAPSLOCKED = !CAPSLOCKED;
                return 0;
            }
            _ => {}
        }
    }

    0
}

/// ---------------------------
/// Keyboard Buffers
/// ---------------------------
pub fn kb_is_occupied() -> bool {
    unsafe { !KB_BUFF.is_null() }
}

pub unsafe fn kb_write_char(c: u8) {
    if KB_BUFF.is_null() || KB_CURR >= KB_MAX {
        return;
    }
    *KB_BUFF.add(KB_CURR) = c;
    KB_CURR += 1;
}

/// ---------------------------
/// IRQ Handler
/// ---------------------------
pub unsafe fn kb_irq() {
    let out = handle_kb_event();
    if out == 0 || KB_BUFF.is_null() {
        return;
    }
    kb_write_char(out);
}

/// ---------------------------
/// Initialize Keyboard
/// ---------------------------
pub unsafe fn initiate_kb() {
    kb_write(0x64, 0xAE); // enable keyboard
    inportb(0x60);        // clear buffer
    SHIFTED = false;
    CAPSLOCKED = false;
    KB_BUFF = core::ptr::null_mut();
    KB_CURR = 0;
    KB_MAX = 0;
}
