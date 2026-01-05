#![no_std]

use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

// ============================
// uACPI basic types
// ============================

type UacpiStatus = i32;
type UacpiPhysAddr = u64;
type UacpiSize = usize;
type UacpiHandle = *mut core::ffi::c_void;
type UacpiThreadId = usize;
type UacpiCpuFlags = i32;

type UacpiU8 = u8;
type UacpiU16 = u16;
type UacpiU32 = u32;
type UacpiU64 = u64;
type UacpiBool = bool;

// ============================
// uACPI constants
// ============================

const UACPI_STATUS_OK: UacpiStatus = 0;
const UACPI_STATUS_INVALID_ARGUMENT: UacpiStatus = -1;
const UACPI_STATUS_TIMEOUT: UacpiStatus = -2;

const UACPI_LOG_DEBUG: u32 = 0;
const UACPI_LOG_TRACE: u32 = 1;
const UACPI_LOG_INFO: u32 = 2;
const UACPI_LOG_WARN: u32 = 3;
const UACPI_LOG_ERROR: u32 = 4;

const UACPI_FIRMWARE_REQUEST_TYPE_BREAKPOINT: u32 = 0;
const UACPI_FIRMWARE_REQUEST_TYPE_FATAL: u32 = 1;

// ============================
// External kernel symbols
// ============================

extern "Rust" {
    static bootloader: Bootloader;
    static timerBootUnix: usize;
    static timerTicks: usize;
    static currentTask: *const Task;

    fn debugf(fmt: &str, ...);
    fn panic() -> !;

    fn malloc(size: usize) -> *mut u8;
    fn calloc(size: usize, count: usize) -> *mut u8;
    fn free(ptr: *mut u8);

    fn inportb(port: u16) -> u8;
    fn inportw(port: u16) -> u16;
    fn inportl(port: u16) -> u32;

    fn outportb(port: u16, value: u64);
    fn outportw(port: u16, value: u64);
    fn outportl(port: u16, value: u64);

    fn handControl();

    fn registerIRQhandler(irq: u32, handler: extern "C" fn()) -> *mut IrqHandler;

    fn checkInterrupts() -> i32;
    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    fn semaphoreWait(sem: *mut Semaphore, timeout: u16) -> bool;
    fn semaphorePost(sem: *mut Semaphore);

    fn ConfigReadWord(bus: u8, dev: u8, func: u8, off: usize) -> u32;
    fn ConfigWriteDword(bus: u8, dev: u8, func: u8, off: usize, val: u32);
}

// ============================
// Kernel structs (opaque)
// ============================

#[repr(C)]
struct Bootloader {
    rsdp: UacpiPhysAddr,
    hhdmOffset: u64,
}

#[repr(C)]
struct Task {
    id: usize,
}

#[repr(C)]
struct IrqHandler {
    argument: usize,
}

type Spinlock = AtomicBool;

#[repr(C)]
struct Semaphore {
    cnt: usize,
    invalid: usize,
    LOCK: Spinlock,
}

#[repr(C)]
struct UacpiFirmwareRequestFatal {
    type_: u32,
    code: u32,
    arg: u32,
}

#[repr(C)]
struct UacpiFirmwareRequest {
    type_: u32,
    fatal: UacpiFirmwareRequestFatal,
}

#[repr(C)]
struct UacpiPciAddress {
    segment: u16,
    bus: u8,
    device: u8,
    function: u8,
}

// ============================
// Required uACPI hooks
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_get_rsdp(out: *mut UacpiPhysAddr) -> UacpiStatus {
    unsafe {
        *out = bootloader.rsdp;
    }
    UACPI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_log(level: u32, msg: *const u8) {
    let lvl = match level {
        UACPI_LOG_DEBUG => "debug",
        UACPI_LOG_TRACE => "trace",
        UACPI_LOG_INFO => "info",
        UACPI_LOG_WARN => "warn",
        UACPI_LOG_ERROR => "error",
        _ => "invalid",
    };

    unsafe {
        debugf("[acpi::%s] %s", lvl, msg);
    }
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_alloc(size: UacpiSize) -> *mut u8 {
    unsafe { malloc(size) }
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_free(mem: *mut u8) {
    unsafe { free(mem) }
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_get_thread_id() -> UacpiThreadId {
    unsafe {
        if currentTask.is_null() {
            0
        } else {
            (*currentTask).id
        }
    }
}

// ============================
// IO space
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_io_map(
    base: u16,
    _len: UacpiSize,
    out: *mut UacpiHandle,
) -> UacpiStatus {
    unsafe { *out = base as usize as UacpiHandle }
    UACPI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_io_unmap(_: UacpiHandle) {}

#[no_mangle]
pub extern "C" fn uacpi_kernel_io_read(
    handle: UacpiHandle,
    offset: UacpiSize,
    width: UacpiU8,
    out: *mut UacpiU64,
) -> UacpiStatus {
    let port = handle as u16 + offset as u16;
    unsafe {
        *out = match width {
            1 => inportb(port) as u64,
            2 => inportw(port) as u64,
            4 => inportl(port) as u64,
            _ => return UACPI_STATUS_INVALID_ARGUMENT,
        };
    }
    UACPI_STATUS_OK
}

// ============================
// Time
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_get_nanoseconds_since_boot() -> UacpiU64 {
    unsafe {
        (timerBootUnix * 1_000_000_000) as u64
            + (timerTicks * 1_000_000) as u64
    }
}

// ============================
// Interrupts
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_install_interrupt_handler(
    irq: UacpiU32,
    handler: extern "C" fn(),
    ctx: UacpiHandle,
    out: *mut UacpiHandle,
) -> UacpiStatus {
    unsafe {
        let h = registerIRQhandler(irq, handler);
        (*h).argument = ctx as usize;
        *out = h as UacpiHandle;
    }
    UACPI_STATUS_OK
}

// ============================
// Spinlocks & mutexes
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_create_spinlock() -> UacpiHandle {
    unsafe { calloc(core::mem::size_of::<Spinlock>(), 1) as UacpiHandle }
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_lock_spinlock(lock: UacpiHandle) -> UacpiCpuFlags {
    unsafe {
        let ints = checkInterrupts();
        spinlockAcquire(lock as *mut Spinlock);
        ints
    }
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_unlock_spinlock(lock: UacpiHandle, state: i32) {
    unsafe {
        spinlockRelease(lock as *mut Spinlock);
        if state != 0 {
            asm!("sti");
        } else {
            asm!("cli");
        }
    }
}

// ============================
// Memory mapping
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_map(addr: UacpiPhysAddr, _: UacpiSize) -> *mut u8 {
    unsafe { (bootloader.hhdmOffset + addr) as *mut u8 }
}

#[no_mangle]
pub extern "C" fn uacpi_kernel_unmap(_: *mut u8, _: UacpiSize) {}

// ============================
// Firmware requests
// ============================

#[no_mangle]
pub extern "C" fn uacpi_kernel_handle_firmware_request(
    req: *mut UacpiFirmwareRequest,
) -> UacpiStatus {
    unsafe {
        if (*req).type_ == UACPI_FIRMWARE_REQUEST_TYPE_FATAL {
            debugf(
                "[acpi::glue] FATAL firmware error type{%d} code{%d} arg{%d}\n",
                (*req).fatal.type_,
                (*req).fatal.code,
                (*req).fatal.arg,
            );
        }
    }
    UACPI_STATUS_OK
}

// ============================
// popcount (compiler runtime)
// ============================

#[no_mangle]
pub extern "C" fn __popcountdi2(a: i64) -> i32 {
    let mut x = a as u64;
    x -= (x >> 1) & 0x5555_5555_5555_5555;
    x = ((x >> 2) & 0x3333_3333_3333_3333) + (x & 0x3333_3333_3333_3333);
    x = (x + (x >> 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    let mut y = (x + (x >> 32)) as u32;
    y += y >> 16;
    ((y + (y >> 8)) & 0x7F) as i32
}
