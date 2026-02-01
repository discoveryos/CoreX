#![no_std]
#![no_main]

use core::panic::PanicInfo;

// -------------------- Syscall numbers --------------------
const SYS_WRITE: usize = 1;
const SYS_EXIT: usize = 60;
const STDOUT: usize = 1;

// -------------------- Syscall wrappers --------------------
unsafe fn write(fd: usize, buf: *const u8, len: usize) -> isize {
    let ret: isize;
    core::arch::asm!(
        "syscall",
        in("rax") SYS_WRITE,
        in("rdi") fd,
        in("rsi") buf,
        in("rdx") len,
        lateout("rax") ret,
        lateout("rcx") _, lateout("r11") _,
    );
    ret
}

unsafe fn exit(code: i32) -> ! {
    core::arch::asm!(
        "syscall",
        in("rax") SYS_EXIT,
        in("rdi") code,
        options(noreturn)
    );
}

// -------------------- Entry point --------------------
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let msg = b"Hello, world!\n";
    unsafe {
        write(STDOUT, msg.as_ptr(), msg.len());
        exit(0);
    }
}

// -------------------- Panic handler --------------------
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { exit(1) }
}
