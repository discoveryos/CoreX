#![no_std]

use core::arch::asm;

// =======================================================
// Basic types
// =======================================================

type Bool = bool;
type U32 = u32;
type U64 = u64;
type SizeT = usize;

// =======================================================
// MSR constants
// =======================================================

const MSRID_STAR: u32 = 0xC000_0081;
const MSRID_LSTAR: u32 = 0xC000_0082;
const MSRID_FMASK: u32 = 0xC000_0084;
const MSRID_EFER: u32 = 0xC000_0080;
const MSRID_KERNEL_GSBASE: u32 = 0xC000_0102;

// =======================================================
// RFLAGS
// =======================================================

const RFLAGS_IF: u64 = 1 << 9;
const RFLAGS_DF: u64 = 1 << 10;

// =======================================================
// Extern kernel symbols
// =======================================================

extern "Rust" {
    fn debugf(fmt: &str, ...);
    fn panic() -> !;

    fn cpuid(eax: *mut U32, ebx: *mut U32, ecx: *mut U32, edx: *mut U32);

    fn rdmsr(msr: U32) -> U64;
    fn wrmsr(msr: U32, val: U64);

    static bootloader: Bootloader;
    static mut threadInfo: ThreadInfo;

    static syscall_entry: extern "C" fn();
}

// =======================================================
// GDT selectors
// =======================================================

const GDT_KERNEL_CODE: U64 = 0x08;
const GDT_USER_CODE: U64 = 0x18;

// =======================================================
// Kernel structs
// =======================================================

#[repr(C)]
struct Bootloader {
    smp: *const SmpCpu,
    smpBspIndex: usize,
}

#[repr(C)]
struct SmpCpu {
    bsp_lapic_id: U32,
}

#[repr(C)]
struct ThreadInfo {
    syscall_stack: U64,
    lapic_id: U32,
}

// =======================================================
// CPUID check
// =======================================================

pub fn check_syscall_inst() -> Bool {
    let mut eax: U32 = 0x8000_0001;
    let mut ebx: U32 = 0;
    let mut ecx: U32 = 0;
    let mut edx: U32 = 0;

    unsafe {
        cpuid(&mut eax, &mut ebx, &mut ecx, &mut edx);
    }

    ((edx >> 11) & 1) != 0
}

// =======================================================
// Initialization
// =======================================================

pub fn initiate_syscall_inst() {
    if !check_syscall_inst() {
        unsafe {
            debugf("[syscalls] FATAL! No support for syscall instruction!\n");
            panic();
        }
    }

    unsafe {
        // Initialize per-core thread info
        threadInfo.syscall_stack = 0;

        let bsp = *bootloader.smp.add(bootloader.smpBspIndex);
        threadInfo.lapic_id = bsp.bsp_lapic_id;

        // Set GS base to threadInfo
        wrmsr(MSRID_KERNEL_GSBASE, &threadInfo as *const _ as SizeT as U64);

        // STAR MSR
        let mut star = rdmsr(MSRID_STAR);
        star &= 0x0000_0000_FFFF_FFFF;

        // USER_CS = GDT_USER_CODE - 16
        // KERNEL_CS = GDT_KERNEL_CODE
        star |= (GDT_USER_CODE - 16) << 48;
        star |= GDT_KERNEL_CODE << 32;

        wrmsr(MSRID_STAR, star);

        // Syscall entry point (from isr.asm)
        wrmsr(MSRID_LSTAR, syscall_entry as U64);

        // Enable SYSCALL/SYSRET in EFER
        let mut efer = rdmsr(MSRID_EFER);
        efer |= 1 << 0;
        wrmsr(MSRID_EFER, efer);

        // Mask IF and DF on syscall entry
        wrmsr(MSRID_FMASK, RFLAGS_IF | RFLAGS_DF);
    }
}
