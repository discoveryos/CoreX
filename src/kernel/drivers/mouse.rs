#![no_std]

use core::cmp::min;
use core::ptr::{copy_nonoverlapping, write_bytes};

//
// ===== External kernel symbols =====
// These must exist elsewhere in your kernel
//

extern "C" {
    fn inportb(port: u16) -> u8;
    fn outportb(port: u16, value: u8);
    fn sleep(ms: u32);

    fn ioApicRedirect(irq: u8, level: bool) -> u8;
    fn registerIRQhandler(irq: u8, handler: extern "C" fn());

    fn devInputEventSetup(name: *const u8) -> *mut DevInputEvent;
    fn inputGenerateEvent(
        dev: *mut DevInputEvent,
        etype: u16,
        code: u16,
        value: i32,
    );
}

//
// ===== Constants =====
//

const MOUSE_STATUS: u16 = 0x64;
const MOUSE_PORT: u16 = 0x60;

const MOUSE_ABIT: u8 = 0x02;
const MOUSE_BBIT: u8 = 0x01;
const MOUSE_WRITE: u8 = 0xD4;

const MOUSE_TIMEOUT: u32 = 100_000;

//
// evdev constants (assumed same values as C)
//

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;

const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;

const SYN_REPORT: u16 = 0;

//
// ===== Kernel structs =====
//

#[repr(C)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
pub struct DevInputEvent {
    pub inputid: InputId,
    pub eventBit: extern "C" fn(*mut OpenFile, u64, *mut u8) -> usize,
}

#[repr(C)]
pub struct OpenFile;

#[repr(C)]
pub struct InputAbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
pub struct Framebuffer {
    pub width: i32,
    pub height: i32,
}

extern "C" {
    static fb: Framebuffer;
}

//
// ===== Global state =====
//

static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE1: u8 = 0;
static mut MOUSE2: u8 = 0;

static mut GX: i32 = 0;
static mut GY: i32 = 0;

static mut CLICKED_LEFT: bool = false;
static mut CLICKED_RIGHT: bool = false;

static mut MOUSE_EVENT: *mut DevInputEvent = core::ptr::null_mut();

//
// ===== Low-level helpers =====
//

unsafe fn mouse_wait(write: bool) {
    let mut timeout = MOUSE_TIMEOUT;
    if !write {
        while timeout > 0 {
            if inportb(MOUSE_STATUS) & MOUSE_BBIT != 0 {
                break;
            }
            timeout -= 1;
        }
    } else {
        while timeout > 0 {
            if inportb(MOUSE_STATUS) & MOUSE_ABIT == 0 {
                break;
            }
            timeout -= 1;
        }
    }
}

unsafe fn mouse_write(value: u8) {
    mouse_wait(true);
    outportb(MOUSE_STATUS, MOUSE_WRITE);
    mouse_wait(true);
    outportb(MOUSE_PORT, value);
}

unsafe fn mouse_read() -> u8 {
    mouse_wait(false);
    inportb(MOUSE_PORT)
}

//
// ===== IRQ handler =====
//

#[no_mangle]
pub extern "C" fn mouse_irq() {
    unsafe {
        let byte = mouse_read();

        match MOUSE_CYCLE {
            0 => {
                // Sync byte
                if byte & (1 << 3) == 0 {
                    MOUSE_CYCLE = 0;
                    return;
                }
                MOUSE1 = byte;
            }
            1 => {
                MOUSE2 = byte;
            }
            _ => {
                let mouse3 = byte;

                let x = MOUSE2 as i8 as i32;
                let y = mouse3 as i8 as i32;

                GX += x;
                GY -= y;

                if GX < 0 {
                    GX = 0;
                }
                if GY < 0 {
                    GY = 0;
                }
                if GX >= fb.width {
                    GX = fb.width - 1;
                }
                if GY >= fb.height {
                    GY = fb.height - 1;
                }

                let left = (MOUSE1 & 0x01) != 0;
                let right = (MOUSE1 & 0x02) != 0;

                if CLICKED_LEFT && !left {
                    inputGenerateEvent(MOUSE_EVENT, EV_KEY, BTN_LEFT, 0);
                }
                if !CLICKED_LEFT && left {
                    inputGenerateEvent(MOUSE_EVENT, EV_KEY, BTN_LEFT, 1);
                }

                if CLICKED_RIGHT && !right {
                    inputGenerateEvent(MOUSE_EVENT, EV_KEY, BTN_RIGHT, 0);
                }
                if !CLICKED_RIGHT && right {
                    inputGenerateEvent(MOUSE_EVENT, EV_KEY, BTN_RIGHT, 1);
                }

                CLICKED_LEFT = left;
                CLICKED_RIGHT = right;

                inputGenerateEvent(MOUSE_EVENT, EV_REL, REL_X, x);
                inputGenerateEvent(MOUSE_EVENT, EV_REL, REL_Y, -y);
                inputGenerateEvent(MOUSE_EVENT, EV_SYN, SYN_REPORT, 0);
            }
        }

        MOUSE_CYCLE = (MOUSE_CYCLE + 1) % 3;
    }
}

//
// ===== ioctl bitmap helpers =====
//

unsafe fn bitmap_set(map: &mut [u8], bit: usize) {
    let idx = bit / 8;
    let off = bit % 8;
    if idx < map.len() {
        map[idx] |= 1 << off;
    }
}

//
// ===== evdev ioctl handler =====
//

#[no_mangle]
pub extern "C" fn mouse_event_bit(
    _fd: *mut OpenFile,
    request: u64,
    arg: *mut u8,
) -> usize {
    unsafe {
        let number = (request & 0xff) as usize;
        let size = ((request >> 16) & 0x3fff) as usize;

        match number {
            0x20 => {
                let out: usize = (1 << EV_SYN) | (1 << EV_KEY) | (1 << EV_REL);
                let len = min(core::mem::size_of::<usize>(), size);
                copy_nonoverlapping(
                    &out as *const _ as *const u8,
                    arg,
                    len,
                );
                len
            }

            0x20 + EV_REL as usize => {
                let out: usize = (1 << REL_X) | (1 << REL_Y);
                let len = min(core::mem::size_of::<usize>(), size);
                copy_nonoverlapping(
                    &out as *const _ as *const u8,
                    arg,
                    len,
                );
                len
            }

            0x20 + EV_KEY as usize => {
                let mut map = [0u8; 96];
                bitmap_set(&mut map, BTN_LEFT as usize);
                bitmap_set(&mut map, BTN_RIGHT as usize);
                let len = min(map.len(), size);
                copy_nonoverlapping(map.as_ptr(), arg, len);
                len
            }

            0x40 + EV_ABS as usize => {
                let info = arg as *mut InputAbsInfo;
                write_bytes(info, 0, 1);
                (*info).minimum = 0;
                (*info).maximum = fb.width;
                0
            }

            _ => 0,
        }
    }
}

//
// ===== Initialization =====
//

pub unsafe fn initiate_mouse() {
    static NAME: &[u8] = b"PS/2 Mouse\0";

    MOUSE_EVENT = devInputEventSetup(NAME.as_ptr());

    (*MOUSE_EVENT).inputid = InputId {
        bustype: 0x05,   // BUS_PS2
        vendor: 0x045e,  // Microsoft
        product: 0x00b4,
        version: 0x0100,
    };

    (*MOUSE_EVENT).eventBit = mouse_event_bit;

    // Enable auxiliary device
    mouse_wait(true);
    outportb(0x64, 0xA8);

    // Enable IRQs
    mouse_wait(true);
    outportb(0x64, 0x20);
    sleep(100);
    mouse_wait(false);
    let status = inportb(0x60) | 2;
    mouse_wait(true);
    outportb(0x64, 0x60);
    mouse_wait(true);
    outportb(0x60, status);

    // Defaults
    mouse_write(0xF6);
    mouse_read();

    // Enable mouse
    mouse_write(0xF4);
    mouse_read();

    let irq = ioApicRedirect(12, false);
    registerIRQhandler(irq, mouse_irq);
}
