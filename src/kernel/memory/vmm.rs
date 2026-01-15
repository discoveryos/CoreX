#![no_std]

use core::ffi::c_void;

//
// Externals & constants
//

const PAGE_SIZE: usize = 4096;
const BLOCK_SIZE: usize = 4096;
const VMM_POS_ENSURE: usize = 0x4000_0000;
const VMM_DEBUG: bool = false;

extern "C" {
    static mut bootloader: BootloaderInfo;
    static mut virtual_mem: DSBitmap;

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn memset(ptr: *mut u8, val: i32, size: usize);

    fn PhysicalAllocate(pages: i32) -> usize;
    fn PhysicalFree(ptr: usize, pages: i32);

    fn VirtualToPhysical(virt: usize) -> usize;
}

//
// Bootloader
//

#[repr(C)]
pub struct BootloaderInfo {
    pub mmTotal: usize,
    pub mmEntryCnt: usize,
    pub mmEntries: *const *const c_void,
    pub hhdmOffset: usize,
}

//
// Bitmap structure (same as PMM)
//

#[repr(C)]
pub struct DSBitmap {
    pub Bitmap: *mut u8,
    pub BitmapSizeInBlocks: usize,
    pub BitmapSizeInBytes: usize,
    pub allocatedSizeInBlocks: usize,
    pub mem_start: usize,
    pub ready: bool,
}

//
// Helpers
//

#[inline(always)]
const fn div_round_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

//
// VMM initialization
//

pub unsafe fn initiate_vmm() {
    let target_position =
        div_round_up(
            bootloader.hhdmOffset - bootloader.mmTotal - VMM_POS_ENSURE,
            PAGE_SIZE,
        ) * PAGE_SIZE;

    virtual_mem.ready = false;
    virtual_mem.mem_start = target_position;

    virtual_mem.BitmapSizeInBlocks =
        div_round_up(bootloader.mmTotal, BLOCK_SIZE);
    virtual_mem.BitmapSizeInBytes =
        div_round_up(virtual_mem.BitmapSizeInBlocks, 8);

    let pages_required =
        div_round_up(virtual_mem.BitmapSizeInBytes, BLOCK_SIZE);

    virtual_mem.Bitmap =
        virtual_allocate(pages_required as i32) as *mut u8;

    memset(
        virtual_mem.Bitmap,
        0,
        virtual_mem.BitmapSizeInBytes,
    );

    // Bitmap must never be mapped inside the virtual allocator
    // (it's in the HHDM region)

    virtual_mem.ready = true;
}

//
// Virtual allocation
//

pub unsafe fn virtual_allocate(pages: i32) -> *mut c_void {
    let phys = PhysicalAllocate(pages);
    let virt = phys + bootloader.hhdmOffset;

    if VMM_DEBUG {
        debugf(
            b"[vmm::alloc] Found region: out{%lx} phys{%lx}\n\0".as_ptr(),
            virt,
            phys,
        );
    }

    virt as *mut c_void
}

//
// Physically contiguous allocation
//

pub unsafe fn virtual_allocate_physically_contiguous(
    pages: i32,
) -> *mut c_void {
    virtual_allocate(pages)
}

//
// Virtual free
//

pub unsafe fn virtual_free(ptr: *mut c_void, pages: i32) -> bool {
    let phys = VirtualToPhysical(ptr as usize);

    if phys == 0 {
        debugf(
            b"[vmm::free] Could not find physical address! virt{%lx}\n\0"
                .as_ptr(),
            ptr as usize,
        );
        panic();
    }

    PhysicalFree(phys, pages);
    true
}
