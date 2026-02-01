#![no_std]
#![no_main]

use core::panic::PanicInfo;

// --------------------- Syscall numbers ---------------------
const SYS_WRITE: usize = 1;
const STDOUT: usize = 1;
const SYS_EXIT: usize = 60;

// --------------------- Helper functions ---------------------
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
        options(noreturn),
    );
}

// --------------------- Main test program ---------------------
#[no_mangle]
pub extern "C" fn main() {
    let msg = b"Hello world!\n";
    unsafe { write(STDOUT, msg.as_ptr(), msg.len()) };
}

// --------------------- _start entry point ---------------------
#[no_mangle]
pub extern "C" fn _start() -> ! {
    main();
    unsafe { exit(0) };
}

// --------------------- Panic handler ---------------------
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { exit(1) };
}
