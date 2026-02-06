// shell.rs

#![no_std]

use core::arch::asm;
use core::ptr;
use core::ffi::c_void;

type bool_t = bool;

#[repr(C)]
pub struct Task {
    pub id: i32,
    pub parent: *mut Task,
    pub state: i32,
    pub waiting_for_pid: i32,
}

#[repr(C)]
pub struct OpenFile {
    pub id: i32,
}

/* =======================
   Extern kernel symbols
   ======================= */

extern "C" {
    // console
    fn printf(fmt: *const u8, ...) -> i32;
    fn clearScreen();
    fn readStr(buf: *mut u8);

    // memory
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    fn memset(ptr: *mut u8, val: i32, size: usize);
    fn snprintf(buf: *mut u8, size: usize, fmt: *const u8, ...) -> i32;

    // disk
    fn getDiskBytes(buf: *mut u8, lba: i32, count: i32);
    fn hexDump(
        title: *const u8,
        data: *const u8,
        size: usize,
        width: usize,
        out: extern "C" fn(*const u8, ...) -> i32,
    );

    // tasking / elf
    fn elfExecute(
        path: *const u8,
        argc: i32,
        argv: *const *const u8,
        a: i32,
        b: i32,
        c: bool,
    ) -> *mut Task;

    fn taskCreateFinish(task: *mut Task);
    fn handControl();

    // fs
    fn fsUserOpen(
        task: *mut Task,
        path: *const u8,
        flags: i32,
        mode: i32,
    ) -> i32;

    fn fsUserGetNode(task: *mut Task, fd: i32) -> *mut OpenFile;

    // misc
    fn panic() -> !;
    fn debugf(fmt: *const u8, ...);

    // globals
    static mut currentTask: *mut Task;
    static timerTicks: u64;
    static bootloader_mmTotal: u64;
}

/* =======================
   Constants
   ======================= */

const SECTOR_SIZE: usize = 512;
const O_RDWR: i32 = 0x2;
const O_APPEND: i32 = 0x400;

const TASK_STATE_WAITING_CHILD_SPECIFIC: i32 = 3;

/* =======================
   Helpers
   ======================= */

#[inline]
fn div_round_up(a: u64, b: u64) -> u64 {
    (a + b - 1) / b
}

extern "C" fn printf_wrapper(fmt: *const u8, ...) -> i32 {
    unsafe { printf(fmt) }
}

/* =======================
   Tasks
   ======================= */

#[no_mangle]
pub extern "C" fn task1() {
    for _ in 0..8 {
        unsafe { printf(b"task 1: aaa\n\0".as_ptr()); }
    }
    unsafe {
        asm!(
            "mov eax, 1",
            "int 0x80",
            options(nostack)
        );
    }
}

#[no_mangle]
pub extern "C" fn task2() {
    for _ in 0..8 {
        unsafe {
            printf(b"task 2: 1111111111111111111111111111111111111\n\0".as_ptr());
        }
    }
    unsafe {
        asm!(
            "mov eax, 1",
            "int 0x80",
            options(nostack)
        );
    }
}

#[no_mangle]
pub extern "C" fn task3() {
    for _ in 0..8 {
        unsafe { printf(b"task 3: 423432423\n\0".as_ptr()); }
    }
    unsafe {
        asm!(
            "mov eax, 1",
            "int 0x80",
            options(nostack)
        );
    }
}

/* =======================
   Shell commands
   ======================= */

#[no_mangle]
pub extern "C" fn fetch() {
    unsafe {
        printf(b"\n      ^         name: cavOS\0".as_ptr());
        printf(
            b"\n     / \\        memory: %ldMB\0".as_ptr(),
            div_round_up(div_round_up(bootloader_mmTotal, 1024), 1024),
        );
        printf(
            b"\n    /   \\       uptime: %lds\0".as_ptr(),
            div_round_up(timerTicks, 1000),
        );
        printf(b"\n   /  ^  \\  _\0".as_ptr());
        printf(b"\n   \\ \\ / / / \\\0".as_ptr());
        printf(b"\n           \\_/  \n\0".as_ptr());
    }
}

#[no_mangle]
pub extern "C" fn help() {
    unsafe {
        printf(b"\n========================== GENERIC ==========================\0".as_ptr());
        printf(b"\n= cmd            : Launch a new recursive Shell             =\0".as_ptr());
        printf(b"\n= clear          : Clears the screen                        =\0".as_ptr());
        printf(b"\n= echo           : Reprintf a given text                    =\0".as_ptr());
        printf(b"\n= exit           : Quits the current shell                  =\0".as_ptr());
        printf(b"\n= fetch          : Brings you some system information       =\0".as_ptr());
        printf(b"\n= time           : Tells you the time and date from BIOS    =\0".as_ptr());
        printf(b"\n= lspci          : Lists PCI device info                    =\0".as_ptr());
        printf(b"\n= dump           : Dumps some of the bitmap allocator       =\0".as_ptr());
        printf(b"\n= draw           : Tests framebuffer by drawing a rectangle =\0".as_ptr());
        printf(b"\n= proctest       : Tests multitasking support               =\0".as_ptr());
        printf(b"\n= exec           : Runs a cavOS binary of your choice       =\0".as_ptr());
        printf(b"\n= bash           : GNU Bash, your portal to userspace!      =\0".as_ptr());
        printf(b"\n=============================================================\n\0".as_ptr());
    }
}

#[no_mangle]
pub extern "C" fn readDisk() {
    unsafe {
        clearScreen();
        printf(b"=========================================\n\0".as_ptr());
        printf(b"====        coreX readdisk 1.0       ====\n\0".as_ptr());
        printf(b"====    Copyright coreX-kern 2026    ====\n\0".as_ptr());
        printf(b"=========================================\n\0".as_ptr());

        printf(b"\nInsert LBA (LBA = Offset / Sector Size): \0".as_ptr());

        let choice = malloc(200);
        readStr(choice);

        let lba = crate::atoi(choice);

        memset(choice, 0, 200);
        snprintf(choice, 200, b"reading disk{0} LBA{%d}:\0".as_ptr(), lba);

        let raw = malloc(SECTOR_SIZE);
        getDiskBytes(raw, lba, 1);

        hexDump(choice, raw, SECTOR_SIZE, 16, printf_wrapper);

        free(raw);
        free(choice);
    }
}

#[no_mangle]
pub extern "C" fn echo(_: *mut u8) {
    unsafe {
        printf(b"\nInsert argument 1: \0".as_ptr());
        let mut buf = [0u8; 200];
        readStr(buf.as_mut_ptr());
        printf(b"\n%s\n\0".as_ptr(), buf.as_ptr());
    }
}

/* =======================
   Process execution
   ======================= */

#[no_mangle]
pub extern "C" fn run(
    binary: *const u8,
    wait: bool,
    argc: i32,
    argv: *const *const u8,
) -> bool_t {
    unsafe {
        let mut task: *mut Task;

        if argc > 0 {
            task = elfExecute(binary, argc, argv, 0, 0, false);
        } else {
            let args = [binary];
            task = elfExecute(binary, 1, args.as_ptr(), 0, 0, false);
        }

        if task.is_null() {
            return false;
        }

        (*task).parent = currentTask;

        let stdin = fsUserOpen(task, b"/dev/stdin\0".as_ptr(), O_RDWR | O_APPEND, 0);
        let stdout = fsUserOpen(task, b"/dev/stdout\0".as_ptr(), O_RDWR | O_APPEND, 0);
        let stderr = fsUserOpen(task, b"/dev/stderr\0".as_ptr(), O_RDWR | O_APPEND, 0);

        if stdin < 0 || stdout < 0 || stderr < 0 {
            debugf(b"[elf] Couldn't establish basic IO!\n\0".as_ptr());
            panic();
        }

        (*fsUserGetNode(task, stdin)).id = 0;
        (*fsUserGetNode(task, stdout)).id = 1;
        (*fsUserGetNode(task, stderr)).id = 2;

        taskCreateFinish(task);

        if wait {
            (*currentTask).waiting_for_pid = (*task).id;
            (*currentTask).state = TASK_STATE_WAITING_CHILD_SPECIFIC;
            handControl();
        }

        true
    }
}

#[no_mangle]
pub extern "C" fn launch_shell(_: i32) {
    unsafe { panic() }
}
