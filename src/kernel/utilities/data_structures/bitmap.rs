#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::mem;

type size_t = usize;
type uint8_t = u8;
type bool_t = bool;

/* ================= CONSTANTS ================= */

const BLOCK_SIZE: size_t = 4096;
const BLOCKS_PER_BYTE: size_t = 8;
const INVALID_BLOCK: size_t = usize::MAX;

/* ================= STRUCT ================= */

#[repr(C)]
pub struct DS_Bitmap {
    pub Bitmap: *mut uint8_t,
    pub BitmapSizeInBlocks: size_t,
    pub BitmapSizeInBytes: size_t,
    pub mem_start: size_t,
    pub allocatedSizeInBlocks: size_t,
    pub lastDeepFragmented: size_t,
}

/* ================= EXTERNS ================= */

extern "C" {
    fn debugf(fmt: *const u8, ...) -> i32;
}

/* ================= UTILS ================= */

#[inline(always)]
fn DivRoundUp(x: size_t, y: size_t) -> size_t {
    (x + y - 1) / y
}

/* ================= CONVERSION ================= */

pub unsafe fn ToPtr(bitmap: *mut DS_Bitmap, block: size_t) -> *mut u8 {
    ((*bitmap).mem_start + block * BLOCK_SIZE) as *mut u8
}

pub unsafe fn ToBlock(bitmap: *mut DS_Bitmap, ptr: *const u8) -> size_t {
    (ptr as size_t - (*bitmap).mem_start) / BLOCK_SIZE
}

pub unsafe fn ToBlockRoundUp(bitmap: *mut DS_Bitmap, ptr: *const u8) -> size_t {
    DivRoundUp(ptr as size_t - (*bitmap).mem_start, BLOCK_SIZE)
}

/* ================= BITMAP CORE ================= */

pub fn BitmapCalculateSize(totalSize: size_t) -> size_t {
    let blocks = DivRoundUp(totalSize, BLOCK_SIZE);
    DivRoundUp(blocks, 8)
}

pub unsafe fn BitmapGet(bitmap: *mut DS_Bitmap, block: size_t) -> i32 {
    let addr = block / BLOCKS_PER_BYTE;
    let offset = block % BLOCKS_PER_BYTE;
    ((*bitmap).Bitmap.add(addr).read() & (1 << offset)) != 0
        as i32
}

pub unsafe fn BitmapSet(bitmap: *mut DS_Bitmap, block: size_t, value: bool_t) {
    let addr = block / BLOCKS_PER_BYTE;
    let offset = block % BLOCKS_PER_BYTE;
    let ptr = (*bitmap).Bitmap.add(addr);

    if value {
        *ptr |= 1 << offset;
    } else {
        *ptr &= !(1 << offset);
    }
}

/* ================= DEBUG ================= */

pub unsafe fn BitmapDump(bitmap: *mut DS_Bitmap) {
    debugf(
        b"=== BYTE DUMPING %d -> %d BYTES ===\n\0".as_ptr(),
        (*bitmap).BitmapSizeInBlocks,
        (*bitmap).BitmapSizeInBytes,
    );

    for i in 0..(*bitmap).BitmapSizeInBytes {
        debugf(b"%x \0".as_ptr(), *(*bitmap).Bitmap.add(i));
    }

    debugf(b"\n\0".as_ptr());
}

pub unsafe fn BitmapDumpBlocks(bitmap: *mut DS_Bitmap) {
    debugf(
        b"=== BLOCK DUMPING %d (512-limited) ===\n\0".as_ptr(),
        (*bitmap).BitmapSizeInBlocks,
    );

    for i in 0..512 {
        debugf(b"%d \0".as_ptr(), BitmapGet(bitmap, i));
    }

    debugf(b"\n\0".as_ptr());
}

/* ================= REGION MARKING ================= */

pub unsafe fn MarkBlocks(
    bitmap: *mut DS_Bitmap,
    start: size_t,
    size: size_t,
    val: bool_t,
) {
    if !val && start < (*bitmap).lastDeepFragmented {
        (*bitmap).lastDeepFragmented = start;
    }

    for i in start..start + size {
        BitmapSet(bitmap, i, val);
    }

    if val {
        (*bitmap).allocatedSizeInBlocks += size;
    } else {
        (*bitmap).allocatedSizeInBlocks -= size;
    }
}

pub unsafe fn MarkRegion(
    bitmap: *mut DS_Bitmap,
    basePtr: *const u8,
    sizeBytes: size_t,
    isUsed: i32,
) {
    let (base, size) = if isUsed != 0 {
        (
            ToBlock(bitmap, basePtr),
            DivRoundUp(sizeBytes, BLOCK_SIZE),
        )
    } else {
        (
            ToBlockRoundUp(bitmap, basePtr),
            sizeBytes / BLOCK_SIZE,
        )
    };

    MarkBlocks(bitmap, base, size, isUsed != 0);
}

/* ================= ALLOCATION ================= */

pub unsafe fn FindFreeRegion(bitmap: *mut DS_Bitmap, blocks: size_t) -> size_t {
    let mut regionStart = (*bitmap).lastDeepFragmented;
    let mut regionSize = 0;

    for i in regionStart..(*bitmap).BitmapSizeInBlocks {
        if BitmapGet(bitmap, i) != 0 {
            regionSize = 0;
            regionStart = i + 1;
        } else {
            if blocks == 1 {
                (*bitmap).lastDeepFragmented = regionStart + 1;
            }

            regionSize += 1;
            if regionSize >= blocks {
                return regionStart;
            }
        }
    }

    debugf(b"[bitmap] Didn't find jack shit!\n\0".as_ptr());
    INVALID_BLOCK
}

pub unsafe fn BitmapAllocate(bitmap: *mut DS_Bitmap, blocks: size_t) -> *mut u8 {
    if blocks == 0 {
        return core::ptr::null_mut();
    }

    let region = FindFreeRegion(bitmap, blocks);
    if region == INVALID_BLOCK {
        return core::ptr::null_mut();
    }

    MarkBlocks(bitmap, region, blocks, true);
    ToPtr(bitmap, region)
}

pub unsafe fn BitmapFree(bitmap: *mut DS_Bitmap, base: *const u8, blocks: size_t) {
    MarkRegion(bitmap, base, BLOCK_SIZE * blocks, 0);
}

/* ================= PAGEFRAMES ================= */

pub unsafe fn BitmapAllocatePageframe(bitmap: *mut DS_Bitmap) -> size_t {
    let region = FindFreeRegion(bitmap, 1);
    MarkBlocks(bitmap, region, 1, true);
    (*bitmap).mem_start + region * BLOCK_SIZE
}

pub unsafe fn BitmapFreePageframe(bitmap: *mut DS_Bitmap, addr: *const u8) {
    MarkRegion(bitmap, addr, BLOCK_SIZE, 0);
}
