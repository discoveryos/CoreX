#![no_std]

use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, Ordering};

//
// Externals & constants (expected to exist elsewhere)
//

const BLOCK_SIZE: usize = 4096;

extern "C" {
    static mut bootloader: BootloaderInfo;

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn memset(ptr: *mut u8, val: i32, size: usize);

    fn BitmapAllocate(bitmap: *mut DSBitmap, pages: i32) -> usize;
    fn MarkRegion(
        bitmap: *mut DSBitmap,
        base: *mut core::ffi::c_void,
        length: usize,
        used: i32,
    );
}

//
// Bootloader / Limine definitions
//

#[repr(C)]
pub struct LimineMemmapEntry {
    pub base: usize,
    pub length: usize,
    pub entry_type: u32,
}

pub const LIMINE_MEMMAP_USABLE: u32 = 0x1;

#[repr(C)]
pub struct BootloaderInfo {
    pub mmTotal: usize,
    pub mmEntryCnt: usize,
    pub mmEntries: *const *const LimineMemmapEntry,
    pub hhdmOffset: usize,
}

//
// Bitmap structure
//

#[repr(C)]
pub struct DSBitmap {
    pub Bitmap: *mut u8,
    pub BitmapSizeInBlocks: usize,
    pub BitmapSizeInBytes: usize,
    pub allocatedSizeInBlocks: usize,
    pub ready: bool,
}

extern "C" {
    static mut physical: DSBitmap;
}

//
// Spinlock
//

pub struct Spinlock {
    locked: AtomicBool,
}

impl Spinlock {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    pub fn acquire(&self) {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {}
    }

    pub fn release(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

static LOCK_PMM: Spinlock = Spinlock::new();

//
// Helpers
//

#[inline(always)]
const fn div_round_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

//
// PMM initialization
//

pub unsafe fn initiate_pmm() {
    let bitmap = &mut physical;
    bitmap.ready = false;

    bitmap.BitmapSizeInBlocks = div_round_up(bootloader.mmTotal, BLOCK_SIZE);
    bitmap.BitmapSizeInBytes = div_round_up(bitmap.BitmapSizeInBlocks, 8);

    let mut chosen_entry: *const LimineMemmapEntry = core::ptr::null();

    for i in 0..bootloader.mmEntryCnt {
        let entry =
            *bootloader.mmEntries.add(i);

        if (*entry).entry_type != LIMINE_MEMMAP_USABLE {
            continue;
        }

        if (*entry).length < bitmap.BitmapSizeInBytes {
            continue;
        }

        chosen_entry = entry;
        break;
    }

    if chosen_entry.is_null() {
        debugf(
            b"[pmm] Not enough memory: required{%lx}!\n\0".as_ptr(),
            bitmap.BitmapSizeInBytes,
        );
        panic();
    }

    let bitmap_start_phys = (*chosen_entry).base;
    bitmap.Bitmap = (bitmap_start_phys + bootloader.hhdmOffset) as *mut u8;

    // Mark everything used initially
    memset(bitmap.Bitmap, 0xFF, bitmap.BitmapSizeInBytes);

    // Mark usable regions free
    for i in 0..bootloader.mmEntryCnt {
        let entry =
            *bootloader.mmEntries.add(i);

        if (*entry).entry_type == LIMINE_MEMMAP_USABLE {
            MarkRegion(
                bitmap,
                (*entry).base as *mut _,
                (*entry).length,
                0,
            );
        }
    }

    // Mark non-usable regions used
    for i in 0..bootloader.mmEntryCnt {
        let entry =
            *bootloader.mmEntries.add(i);

        if (*entry).entry_type != LIMINE_MEMMAP_USABLE {
            MarkRegion(
                bitmap,
                (*entry).base as *mut _,
                (*entry).length,
                1,
            );
        }
    }

    // Reserve bitmap memory itself
    MarkRegion(
        bitmap,
        bitmap_start_phys as *mut _,
        bitmap.BitmapSizeInBytes,
        1,
    );

    bitmap.allocatedSizeInBlocks = 0;

    debugf(
        b"[pmm] Bitmap initiated: bitmapStartPhys{0x%lx} size{%lx}\n\0".as_ptr(),
        bitmap_start_phys,
        bitmap.BitmapSizeInBytes,
    );

    bitmap.ready = true;
}

//
// Physical allocation
//

pub unsafe fn physical_allocate(pages: i32) -> usize {
    LOCK_PMM.acquire();
    let phys = BitmapAllocate(&mut physical, pages);
    LOCK_PMM.release();

    if phys == 0 {
        debugf(
            b"[vmm::alloc] Physical kernel memory ran out!\n\0".as_ptr(),
        );
        panic();
    }

    phys
}

pub unsafe fn physical_free(ptr: usize, pages: i32) {
    LOCK_PMM.acquire();
    MarkRegion(
        &mut physical,
        ptr as *mut _,
        pages as usize * BLOCK_SIZE,
        0,
    );
    LOCK_PMM.release();
}
