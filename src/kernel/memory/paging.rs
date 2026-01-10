#![no_std]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(unused_variables)]

use core::ptr::{null_mut};
use core::sync::atomic::{AtomicUsize, Ordering};

//
// ======================
// Constants & Macros
// ======================
//

const PAGE_SIZE: usize = 4096;
const PAGE_MASK_4K: usize = 0xFFF;

const PF_PRESENT: u64 = 1 << 0;
const PF_RW: u64      = 1 << 1;
const PF_USER: u64    = 1 << 2;
const PF_PS: u64      = 1 << 7;
const PF_SHARED: u64  = 1 << 9;

#[inline(always)]
const fn PML4E(v: u64) -> usize { ((v >> 39) & 0x1FF) as usize }
#[inline(always)]
const fn PDPTE(v: u64) -> usize { ((v >> 30) & 0x1FF) as usize }
#[inline(always)]
const fn PDE(v: u64) -> usize   { ((v >> 21) & 0x1FF) as usize }
#[inline(always)]
const fn PTE(v: u64) -> usize   { ((v >> 12) & 0x1FF) as usize }

#[inline(always)]
const fn PTE_GET_ADDR(x: u64) -> u64 { x & 0x000FFFFFFFFFF000 }

#[inline(always)]
const fn AMD64_MM_STRIPSX(v: u64) -> u64 { v & 0x0000FFFFFFFFFFFF }

#[inline(always)]
const fn DivRoundUp(a: u64, b: u64) -> u64 { (a + b - 1) / b }

#[inline(always)]
const fn BITS_TO_VIRT_ADDR(pml4: usize, pdp: usize, pd: usize, pt: usize) -> u64 {
    ((pml4 as u64) << 39) |
    ((pdp  as u64) << 30) |
    ((pd   as u64) << 21) |
    ((pt   as u64) << 12)
}

//
// ======================
// Global stubs
// ======================
//

#[repr(C)]
pub struct Bootloader {
    pub hhdmOffset: u64,
    pub mmTotal: u64,
}

static mut bootloader: Bootloader = Bootloader {
    hhdmOffset: 0xFFFF800000000000,
    mmTotal: 0x100000000,
};

#[repr(C)]
pub struct Framebuffer {
    pub phys: u64,
    pub width: u64,
    pub height: u64,
}

static fb: Framebuffer = Framebuffer {
    phys: 0,
    width: 0,
    height: 0,
};

//
// ======================
// Spinlock (minimal)
// ======================
//

pub struct SpinlockCnt {
    v: AtomicUsize,
}

impl SpinlockCnt {
    pub const fn new() -> Self {
        Self { v: AtomicUsize::new(0) }
    }
}

fn spinlockCntWriteAcquire(_: &SpinlockCnt) {}
fn spinlockCntWriteRelease(_: &SpinlockCnt) {}
fn spinlockCntReadAcquire(_: &SpinlockCnt) {}
fn spinlockCntReadRelease(_: &SpinlockCnt) {}

static WLOCK_PAGING: SpinlockCnt = SpinlockCnt::new();

//
// ======================
// Memory / Task stubs
// ======================
//

fn PhysicalAllocate(_pages: usize) -> u64 { 0x100000 }
fn PhysicalFree(_addr: u64, _pages: usize) {}

fn VirtualAllocate(_pages: usize) -> *mut u64 {
    unsafe {
        static mut BUF: [u64; 512] = [0; 512];
        BUF.as_mut_ptr()
    }
}

fn panic() -> ! {
    loop {}
}

//
// ======================
// Paging globals
// ======================
//

static mut globalPagedir: *mut u64 = null_mut();

//
// ======================
// Paging core
// ======================
//

pub unsafe fn initiatePaging() {
    let mut cr3: u64;
    core::arch::asm!("mov {}, cr3", out(reg) cr3);
    if cr3 == 0 {
        panic();
    }
    globalPagedir = (cr3 + bootloader.hhdmOffset) as *mut u64;
}

pub unsafe fn VirtualMapRegionByLength(
    virt: u64,
    phys: u64,
    len: u64,
    flags: u64,
) {
    let pages = DivRoundUp(len, PAGE_SIZE as u64);
    for i in 0..pages {
        VirtualMap(
            virt + i * PAGE_SIZE as u64,
            phys + i * PAGE_SIZE as u64,
            flags,
        );
    }
}

pub unsafe fn ChangePageDirectoryUnsafe(pd: *mut u64) {
    let phys = VirtualToPhysical(pd as usize);
    if phys == 0 {
        panic();
    }
    core::arch::asm!("mov cr3, {}", in(reg) phys);
    globalPagedir = pd;
}

pub unsafe fn VirtualMap(virt: u64, phys: u64, flags: u64) {
    VirtualMapL(globalPagedir, virt, phys, flags);
}

pub unsafe fn PagingPhysAllocate() -> u64 {
    let phys = PhysicalAllocate(1);
    let virt = (phys + bootloader.hhdmOffset) as *mut u8;
    core::ptr::write_bytes(virt, 0, PAGE_SIZE);
    phys
}

pub unsafe fn VirtualMapL(
    pagedir: *mut u64,
    virt: u64,
    phys: u64,
    flags: u64,
) {
    if virt & PAGE_MASK_4K as u64 != 0 {
        panic();
    }

    let v = AMD64_MM_STRIPSX(virt);

    let pml4 = &mut *pagedir.add(PML4E(v));
    spinlockCntWriteAcquire(&WLOCK_PAGING);

    if *pml4 & PF_PRESENT == 0 {
        *pml4 = PagingPhysAllocate() | PF_PRESENT | PF_RW | PF_USER;
    }

    let pdp = (PTE_GET_ADDR(*pml4) + bootloader.hhdmOffset) as *mut u64;
    let pdpte = &mut *pdp.add(PDPTE(v));

    if *pdpte & PF_PRESENT == 0 {
        *pdpte = PagingPhysAllocate() | PF_PRESENT | PF_RW | PF_USER;
    }

    let pd = (PTE_GET_ADDR(*pdpte) + bootloader.hhdmOffset) as *mut u64;
    let pde = &mut *pd.add(PDE(v));

    if *pde & PF_PRESENT == 0 {
        *pde = PagingPhysAllocate() | PF_PRESENT | PF_RW | PF_USER;
    }

    let pt = (PTE_GET_ADDR(*pde) + bootloader.hhdmOffset) as *mut u64;
    let pte = &mut *pt.add(PTE(v));

    if phys == 0 {
        *pte = 0;
    } else {
        *pte = (phys & 0x000FFFFFFFFFF000) | PF_PRESENT | flags;
    }

    core::arch::asm!("invlpg [{}]", in(reg) virt);
    spinlockCntWriteRelease(&WLOCK_PAGING);
}

pub unsafe fn VirtualToPhysicalL(pagedir: *mut u64, virt: usize) -> usize {
    if pagedir.is_null() {
        return 0;
    }

    let v = AMD64_MM_STRIPSX((virt & !PAGE_MASK_4K) as u64);

    let pml4 = *pagedir.add(PML4E(v));
    if pml4 & PF_PRESENT == 0 { return 0; }

    let pdp = *((PTE_GET_ADDR(pml4) + bootloader.hhdmOffset) as *const u64)
        .add(PDPTE(v));
    if pdp & PF_PRESENT == 0 { return 0; }

    let pd = *((PTE_GET_ADDR(pdp) + bootloader.hhdmOffset) as *const u64)
        .add(PDE(v));
    if pd & PF_PRESENT == 0 { return 0; }

    let pt = *((PTE_GET_ADDR(pd) + bootloader.hhdmOffset) as *const u64)
        .add(PTE(v));
    if pt & PF_PRESENT == 0 { return 0; }

    (PTE_GET_ADDR(pt) as usize) | (virt & PAGE_MASK_4K)
}

pub unsafe fn VirtualToPhysical(virt: usize) -> usize {
    VirtualToPhysicalL(globalPagedir, virt)
}
