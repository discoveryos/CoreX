#![no_std]
#![no_main]

use core::arch::asm;

/* ===== Extern main ===== */

extern "C" {
    fn main(argc: isize, argv: *const *const u8) -> isize;
}

/* ===== Syscall numbers ===== */

const SYSCALL_GETARGC: u64 = 6;
const SYSCALL_GETARGV: u64 = 7;
const SYSCALL_EXIT_TASK: u64 = 1;

/* ===== Minimal syscall helpers ===== */

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

/* ===== Syscall wrappers ===== */

#[inline(always)]
fn syscall_getargc() -> usize {
    unsafe { syscall0(SYSCALL_GETARGC) as usize }
}

#[inline(always)]
fn syscall_getargv(i: usize) -> *const u8 {
    unsafe { syscall1(SYSCALL_GETARGV, i as u64) as *const u8 }
}

#[inline(always)]
fn syscall_exit_task(code: isize) -> ! {
    unsafe {
        syscall1(SYSCALL_EXIT_TASK, code as u64);
    }
    loop {
        unsafe { core::hint::unreachable_unchecked() }
    }
}

/* ===== Entry point ===== */

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let argc = syscall_getargc();

    // Stack-allocated argv array (same behavior as your C VLA)
    let mut argv: [*const u8; 64] = [core::ptr::null(); 64];

    // Safety guard: prevent overflow if kernel lies
    let argc = core::cmp::min(argc, argv.len());

    for i in 0..argc {
        argv[i] = syscall_getargv(i);
    }

    let ret = unsafe { main(argc as isize, argv.as_ptr()) };
    syscall_exit_task(ret);
}
