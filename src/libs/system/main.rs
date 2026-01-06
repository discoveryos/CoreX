#![no_std]

use core::arch::asm;

pub const SYSCALL_TEST: u64 = 0;
pub const SYSCALL_EXIT_TASK: u64 = 1;
pub const SYSCALL_FORK: u64 = 2;
pub const SYSCALL_READ: u64 = 3;
pub const SYSCALL_WRITE: u64 = 4;
pub const SYSCALL_GETPID: u64 = 5;
pub const SYSCALL_GETARGC: u64 = 6;
pub const SYSCALL_GETARGV: u64 = 7;
pub const SYSCALL_GET_HEAP_START: u64 = 8;
pub const SYSCALL_GET_HEAP_END: u64 = 9;
pub const SYSCALL_ADJUST_HEAP_END: u64 = 10;

#[inline(always)]
unsafe fn syscall0(n: u64) -> u64 {
    let ret: u64;
    asm!(
        "syscall",
        in("rax") n,
        lateout("rax") ret,
        clobber_abi("sysv64"),
    );
    ret
}

#[inline(always)]
unsafe fn syscall1(n: u64, a1: u64) -> u64 {
    let ret: u64;
    asm!(
        "syscall",
        in("rax") n,
        in("rdi") a1,
        lateout("rax") ret,
        clobber_abi("sysv64"),
    );
    ret
}

#[inline(always)]
unsafe fn syscall3(n: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    let ret: u64;
    asm!(
        "syscall",
        in("rax") n,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rax") ret,
        clobber_abi("sysv64"),
    );
    ret
}

/* ===== Public syscall wrappers ===== */

pub fn syscall_test(msg: *const u8) {
    unsafe {
        syscall1(SYSCALL_TEST, msg as u64);
    }
}

pub fn syscall_exit_task(return_code: i32) -> ! {
    unsafe {
        syscall1(SYSCALL_EXIT_TASK, return_code as u64);
    }
    loop {
        core::hint::unreachable_unchecked();
    }
}

pub fn syscall_fork() -> u64 {
    unsafe { syscall0(SYSCALL_FORK) }
}

pub fn syscall_read(fd: u64, buf: *mut u8, count: u64) -> u64 {
    unsafe { syscall3(SYSCALL_READ, fd, buf as u64, count) }
}

pub fn syscall_write(fd: u64, buf: *const u8, count: u64) -> u64 {
    unsafe { syscall3(SYSCALL_WRITE, fd, buf as u64, count) }
}

pub fn syscall_getpid() -> u64 {
    unsafe { syscall0(SYSCALL_GETPID) }
}

pub fn syscall_getargc() -> u64 {
    unsafe { syscall0(SYSCALL_GETARGC) }
}

pub fn syscall_getargv(index: u64) -> *const u8 {
    unsafe { syscall1(SYSCALL_GETARGV, index) as *const u8 }
}

pub fn syscall_get_heap_start() -> u64 {
    unsafe { syscall0(SYSCALL_GET_HEAP_START) }
}

pub fn syscall_get_heap_end() -> u64 {
    unsafe { syscall0(SYSCALL_GET_HEAP_END) }
}

pub fn syscall_adjust_heap_end(heap_end: u64) {
    unsafe {
        syscall1(SYSCALL_ADJUST_HEAP_END, heap_end);
    }
}
