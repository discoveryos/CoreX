#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Serial port constants
const COM1: u16 = 0x3F8;
const COM2: u16 = 0x2F8;
const COM3: u16 = 0x3E8;
const COM4: u16 = 0x2E8;

// ---- Port I/O ----
unsafe fn inportb(port: u16) -> u8 {
    let ret: u8;
    core::arch::asm!("inb %dx, %al", out("al") ret, in("dx") port);
    ret
}

unsafe fn outportb(port: u16, data: u8) {
    core::arch::asm!("outb %al, %dx", in("dx") port, in("al") data);
}

// ---- Serial ----
unsafe fn serial_rcvd(device: u16) -> bool {
    inportb(device + 5) & 1 != 0
}

unsafe fn serial_recv(device: u16) -> u8 {
    while !serial_rcvd(device) {}
    inportb(device)
}

unsafe fn serial_recv_async(device: u16) -> u8 {
    inportb(device)
}

unsafe fn serial_transmit_empty(device: u16) -> bool {
    inportb(device + 5) & 0x20 != 0
}

unsafe fn serial_send(device: u16, out: u8) {
    while !serial_transmit_empty(device) {}
    outportb(device, out);
}

// ---- String utils ----
fn strlength(s: &str) -> usize {
    s.len()
}

fn atoi(s: &str) -> i32 {
    s.bytes().fold(0, |acc, b| acc * 10 + (b - b'0') as i32)
}

fn reverse(s: &mut [u8]) {
    let mut i = 0;
    let mut j = s.len() - 1;
    while i < j {
        s.swap(i, j);
        i += 1;
        j -= 1;
    }
}

fn itoa(mut n: u64, s: &mut [u8]) -> usize {
    let mut i = 0;
    if n == 0 {
        s[0] = b'0';
        return 1;
    }
    while n > 0 {
        s[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    s[..i].reverse();
    i
}

// ---- Syscalls ----
unsafe fn syscall0(n: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        in("rax") n,
        out("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

unsafe fn syscall1(n: u64, a1: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        in("rax") n,
        in("rdi") a1,
        out("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

unsafe fn syscall3(n: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        in("rax") n,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        out("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

// ---- Printing ----
fn print_num(num: u64) {
    let mut buf = [0u8; 50];
    let len = itoa(num, &mut buf);
    unsafe { syscall3(1, 1, buf.as_ptr() as u64, len as u64) };
}

// ---- Main ----
#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let nl = b"\n";
    for i in 0..argc {
        unsafe {
            let arg_ptr = *argv.offset(i as isize);
            let len = {
                let mut len = 0;
                while *arg_ptr.offset(len) != 0 { len += 1; }
                len
            };
            syscall3(1, 1, arg_ptr as u64, len as u64);
            syscall3(1, 1, nl.as_ptr() as u64, 1);
        }
    }
    0
}

// ---- Entry Point ----
#[no_mangle]
pub extern "C" fn _start_c(rsp: u64) -> ! {
    let argc = unsafe { *(rsp as *const i32) };
    let argv = unsafe { (rsp + 8) as *const *const u8 };
    main(argc, argv);
    unsafe { syscall1(60, 0) }; // exit(0)
    loop {}
}

// ---- Panic Handler ----
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
