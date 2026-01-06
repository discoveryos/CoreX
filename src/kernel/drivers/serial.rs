#![no_std]
#![feature(c_variadic)]

use core::ffi::VaList;

//
// ===== Externs from kernel =====
//

extern "C" {
    fn outportb(port: u16, value: u8);
    fn inportb(port: u16) -> u8;

    fn checkInterrupts() -> bool;

    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    fn vfctprintf(
        putc: extern "C" fn(u8, *mut core::ffi::c_void),
        arg: *mut core::ffi::c_void,
        format: *const u8,
        args: VaList,
    ) -> i32;
}

//
// ===== Constants =====
//

const COM1: u16 = 0x3F8;
// const COM2: u16 = 0x2F8;

//
// ===== Spinlock =====
//

#[repr(C)]
pub struct Spinlock {
    pub locked: u32,
}

static mut LOCK_DEBUGF: Spinlock = Spinlock { locked: 0 };

//
// ===== Serial driver =====
//

#[no_mangle]
pub extern "C" fn serial_enable(device: u16) {
    unsafe {
        outportb(device + 1, 0x00);
        outportb(device + 3, 0x80); // Enable divisor mode
        outportb(device + 0, 0x03); // Div Low: 38400 baud
        outportb(device + 1, 0x00); // Div High
        outportb(device + 3, 0x03);
        outportb(device + 2, 0xC7);
        outportb(device + 4, 0x0B);
    }
}

#[no_mangle]
pub extern "C" fn initiateSerial() {
    unsafe {
        debugf(b"[serial] Installing serial...\n\0".as_ptr());
        serial_enable(COM1);
        outportb(COM1 + 1, 0x01);
    }
}

//
// ===== Serial I/O =====
//

#[inline]
fn serial_rcvd(device: u16) -> bool {
    unsafe { inportb(device + 5) & 1 != 0 }
}

#[no_mangle]
pub extern "C" fn serial_recv(device: u16) -> u8 {
    while !serial_rcvd(device) {}
    unsafe { inportb(device) }
}

#[no_mangle]
pub extern "C" fn serial_recv_async(device: u16) -> u8 {
    unsafe { inportb(device) }
}

#[inline]
fn serial_transmit_empty(device: u16) -> bool {
    unsafe { inportb(device + 5) & 0x20 != 0 }
}

#[no_mangle]
pub extern "C" fn serial_send(device: u16, out: u8) {
    while !serial_transmit_empty(device) {}
    unsafe { outportb(device, out) }
}

//
// ===== debugf backend =====
//

extern "C" fn debug_putchar(c: u8, _arg: *mut core::ffi::c_void) {
    unsafe {
        serial_send(COM1, c);
    }
}

//
// ===== debugf =====
//

#[no_mangle]
pub unsafe extern "C" fn debugf(format: *const u8, mut args: ...) -> i32 {
    let ints = checkInterrupts();

    if ints {
        spinlockAcquire(&mut LOCK_DEBUGF);
    }

    let ret = vfctprintf(
        debug_putchar,
        core::ptr::null_mut(),
        format,
        args.as_va_list(),
    );

    if ints {
        spinlockRelease(&mut LOCK_DEBUGF);
    }

    ret
}
