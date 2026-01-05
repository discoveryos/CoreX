#![no_std]

use core::arch::asm;
use core::mem::size_of;

// ======================================================
// Basic types
// ======================================================

type U8 = u8;
type U16 = u16;
type U32 = u32;
type U64 = u64;
type SizeT = usize;

// ======================================================
// Extern kernel symbols
// ======================================================

extern "Rust" {
    fn debugf(fmt: &str, ...);
    fn panic() -> !;

    fn cpuid(eax: *mut u32, ebx: *mut u32, ecx: *mut u32, edx: *mut u32);

    fn rdmsr(msr: u32) -> u64;
    fn wrmsr(msr: u32, val: u64);

    fn memset(ptr: *mut u8, val: i32, size: usize);

    static bootloader: Bootloader;
    static mut threadInfo: ThreadInfo;

    fn syscall_entry();
}

// ======================================================
// MSR constants
// ======================================================

const MSRID_STAR: u32 = 0xC000_0081;
const MSRID_LSTAR: u32 = 0xC000_0082;
const MSRID_FMASK: u32 = 0xC000_0084;
const MSRID_EFER: u32 = 0xC000_0080;
const MSRID_KERNEL_GSBASE: u32 = 0xC000_0102;

const RFLAGS_IF: u64 = 1 << 9;
const RFLAGS_DF: u64 = 1 << 10;

// ======================================================
// GDT selectors (must match layout)
// ======================================================

const GDT_KERNEL_CODE: u64 = 0x28;
const GDT_KERNEL_DATA: u64 = 0x30;
const GDT_USER_CODE: u64 = 0x48;

// ======================================================
// Bootloader / SMP structs (partial)
// ======================================================

#[repr(C)]
struct Bootloader {
    smp: *const SmpInfo,
    smpBspIndex: usize,
}

#[repr(C)]
struct SmpInfo {
    bsp_lapic_id: u32,
}

// ======================================================
// Thread-local syscall info
// ======================================================

#[repr(C)]
struct ThreadInfo {
    syscall_stack: u64,
    lapic_id: u32,
}

// ======================================================
// GDT / TSS structures
// ======================================================

#[repr(C, packed)]
struct GDTEntry {
    limit: U16,
    base_low: U16,
    base_mid: U8,
    access: U8,
    granularity: U8,
    base_high: U8,
}

#[repr(C, packed)]
struct TSSDescriptor {
    length: U16,
    base_low: U16,
    base_mid: U8,
    flags1: U8,
    flags2: U8,
    base_high: U8,
    base_upper32: U32,
    reserved: U32,
}

#[repr(C, packed)]
struct GDTEntries {
    descriptors: [GDTEntry; 11],
    tss: TSSDescriptor,
}

#[repr(C, packed)]
struct GDTPtr {
    limit: U16,
    base: U64,
}

#[repr(C)]
struct TSSPtr {
    _reserved: [u8; 104],
}

// ======================================================
// Statics
// ======================================================

static mut GDT: GDTEntries = unsafe { core::mem::zeroed() };
static mut GDTR: GDTPtr = GDTPtr { limit: 0, base: 0 };
static mut TSS: TSSPtr = TSSPtr { _reserved: [0; 104] };

pub static mut TSS_PTR: *mut TSSPtr = unsafe { &mut TSS };

// ======================================================
// SYSCALL support
// ======================================================

pub fn check_syscall_inst() -> bool {
    let mut eax = 0x8000_0001;
    let mut ebx = 0;
    let mut ecx = 0;
    let mut edx = 0;

    unsafe { cpuid(&mut eax, &mut ebx, &mut ecx, &mut edx) };
    ((edx >> 11) & 1) != 0
}

pub fn initiate_syscall_inst() {
    if !check_syscall_inst() {
        unsafe {
            debugf("[syscalls] FATAL! No syscall instruction support!\n");
            panic();
        }
    }

    unsafe {
        threadInfo.syscall_stack = 0;
        threadInfo.lapic_id =
            (*bootloader.smp.add(bootloader.smpBspIndex)).bsp_lapic_id;

        wrmsr(MSRID_KERNEL_GSBASE, &threadInfo as *const _ as u64);

        let mut star = rdmsr(MSRID_STAR) & 0x0000_0000_FFFF_FFFF;
        star |= (GDT_USER_CODE - 16) << 48;
        star |= GDT_KERNEL_CODE << 32;
        wrmsr(MSRID_STAR, star);

        wrmsr(MSRID_LSTAR, syscall_entry as u64);

        let mut efer = rdmsr(MSRID_EFER);
        efer |= 1;
        wrmsr(MSRID_EFER, efer);

        wrmsr(MSRID_FMASK, RFLAGS_IF | RFLAGS_DF);
    }
}

// ======================================================
// GDT / TSS
// ======================================================

unsafe fn gdt_load_tss(tss: *const TSSPtr) {
    let addr = tss as u64;

    GDT.tss.base_low = addr as u16;
    GDT.tss.base_mid = (addr >> 16) as u8;
    GDT.tss.flags1 = 0b1000_1001;
    GDT.tss.flags2 = 0;
    GDT.tss.base_high = (addr >> 24) as u8;
    GDT.tss.base_upper32 = (addr >> 32) as u32;
    GDT.tss.reserved = 0;

    asm!("ltr {0:x}", in(reg) 0x58u16, options(nostack, preserves_flags));
}

unsafe fn gdt_reload() {
    asm!(
        "lgdt [{0}]",
        "push {1}",
        "lea rax, [rip + 1f]",
        "push rax",
        "lretq",
        "1:",
        "mov ax, {2}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        in(reg) &GDTR,
        const 0x28u64,
        const 0x30u16,
        out("rax") _,
        options(nostack)
    );
}

pub fn initiate_gdt() {
    unsafe {
        // Null
        GDT.descriptors[0] = core::mem::zeroed();

        // Kernel code/data (16/32/64) — matches your C exactly
        GDT.descriptors[1] = GDTEntry { limit: 0xffff, base_low: 0, base_mid: 0, access: 0x9A, granularity: 0, base_high: 0 };
        GDT.descriptors[2] = GDTEntry { limit: 0xffff, base_low: 0, base_mid: 0, access: 0x92, granularity: 0, base_high: 0 };
        GDT.descriptors[3] = GDTEntry { limit: 0xffff, base_low: 0, base_mid: 0, access: 0x9A, granularity: 0xCF, base_high: 0 };
        GDT.descriptors[4] = GDTEntry { limit: 0xffff, base_low: 0, base_mid: 0, access: 0x92, granularity: 0xCF, base_high: 0 };
        GDT.descriptors[5] = GDTEntry { limit: 0, base_low: 0, base_mid: 0, access: 0x9A, granularity: 0x20, base_high: 0 };
        GDT.descriptors[6] = GDTEntry { limit: 0, base_low: 0, base_mid: 0, access: 0x92, granularity: 0, base_high: 0 };

        GDT.descriptors[7] = core::mem::zeroed();
        GDT.descriptors[8] = core::mem::zeroed();

        GDT.descriptors[10] = GDTEntry { limit: 0, base_low: 0, base_mid: 0, access: 0xFA, granularity: 0x20, base_high: 0 };
        GDT.descriptors[9]  = GDTEntry { limit: 0, base_low: 0, base_mid: 0, access: 0xF2, granularity: 0, base_high: 0 };

        GDT.tss.length = 104;

        GDTR.limit = (size_of::<GDTEntries>() - 1) as u16;
        GDTR.base = &GDT as *const _ as u64;

        gdt_reload();

        memset(&mut TSS as *mut _ as *mut u8, 0, size_of::<TSSPtr>());
        gdt_load_tss(&TSS);
    }
}
