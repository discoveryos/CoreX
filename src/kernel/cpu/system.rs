#![no_std]

use core::arch::asm;

// ======================================================
// Externals
// ======================================================

extern "C" {
    fn debugf(fmt: *const u8, ...);
}

// ======================================================
// CPUID
// ======================================================

pub unsafe fn cpuid(eax: &mut u32, ebx: &mut u32, ecx: &mut u32, edx: &mut u32) {
    asm!(
        "cpuid",
        inout("eax") *eax,
        lateout("ebx") *ebx,
        inout("ecx") *ecx,
        lateout("edx") *edx,
        options(nostack)
    );
}

// ======================================================
// Port I/O
// ======================================================

#[inline]
pub unsafe fn inportb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack));
    value
}

#[inline]
pub unsafe fn outportb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack));
}

#[inline]
pub unsafe fn inportw(port: u16) -> u16 {
    let value: u16;
    asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack));
    value
}

#[inline]
pub unsafe fn outportw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack));
}

#[inline]
pub unsafe fn inportl(port: u16) -> u32 {
    let value: u32;
    asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack));
    value
}

#[inline]
pub unsafe fn outportl(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack));
}

// ======================================================
// MSRs
// ======================================================

#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nostack)
    );
    ((high as u64) << 32) | (low as u64)
}

#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nostack)
    );
}

// ======================================================
// CPU feature checks
// ======================================================

pub unsafe fn check_sse() -> bool {
    let mut eax = 1;
    let mut ebx = 0;
    let mut ecx = 0;
    let mut edx = 0;
    cpuid(&mut eax, &mut ebx, &mut ecx, &mut edx);
    (edx & (1 << 25)) != 0
}

pub unsafe fn check_fxsr() -> bool {
    let mut eax = 1;
    let mut ebx = 0;
    let mut ecx = 0;
    let mut edx = 0;
    cpuid(&mut eax, &mut ebx, &mut ecx, &mut edx);
    (edx & (1 << 24)) != 0
}

// ======================================================
// SSE / AVX enable
// ======================================================

pub unsafe fn initiate_sse() {
    if !check_sse() {
        debugf(b"[sse] FATAL! No SSE support!\0".as_ptr());
        panic();
    }

    if !check_fxsr() {
        debugf(b"[sse] FATAL! No FXSR support!\0".as_ptr());
        panic();
    }

    // Enable SSE
    asm!(
        "
        mov rax, cr0
        and ax, 0xFFFB
        or eax, 2
        mov cr0, rax

        mov rax, cr4
        or rax, 0b11000000000
        mov cr4, rax
        ",
        out("rax") _
    );

    // Enable NE + reset FPU
    asm!(
        "
        fninit
        mov rax, cr0
        or rax, 0b100000
        mov cr0, rax
        ",
        out("rax") _
    );

    // XSAVE / AVX
    let mut eax = 1;
    let mut ebx = 0;
    let mut ecx = 0;
    let mut edx = 0;
    cpuid(&mut eax, &mut ebx, &mut ecx, &mut edx);

    if (ecx & (1 << 26)) != 0 {
        debugf(b"[cpu] XSAVE available, enabling\n\0".as_ptr());

        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4);
        cr4 |= 1 << 18;
        asm!("mov cr4, {}", in(reg) cr4);

        if (ecx & (1 << 28)) != 0 {
            debugf(b"[cpu] AVX available, enabling\n\0".as_ptr());
            asm!(
                "xsetbv",
                in("ecx") 0,
                in("eax") 0x7u32,
                in("edx") 0u32
            );
        }
    }

    debugf(b"[cpu] CPU features enabled\n\0".as_ptr());
}

// ======================================================
// Panic / assert
// ======================================================

#[inline(always)]
pub fn panic() -> ! {
    unsafe {
        debugf(b"[kernel] Kernel panic triggered!\n\0".as_ptr());
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}

#[inline(always)]
pub fn assert(expr: bool) {
    if !expr {
        panic();
    }
}

// ======================================================
// Interrupt flag
// ======================================================

pub unsafe fn check_interrupts() -> bool {
    let flags: u64;
    asm!("pushfq; pop {}", out(reg) flags, options(nostack));
    (flags & (1 << 9)) != 0
}

// ======================================================
// Endianness
// ======================================================

#[inline]
pub fn switch_endian_16(val: u16) -> u16 {
    (val << 8) | (val >> 8)
}

#[inline]
pub fn switch_endian_32(val: u32) -> u32 {
    val.rotate_left(24)
}
