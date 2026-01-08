#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;

//
// Constants
//

const SECTOR_SIZE: usize = 512;
const FAT32_CACHE_BAD: u32 = 0xFFFFFFFF;
const FAT32_CACHE_MAX: usize = 8; // must match fat32.h

//
// Structs
//

#[repr(C)]
pub struct FAT32 {
    pub offsetFats: u32,

    pub cacheBase: [u32; FAT32_CACHE_MAX],
    pub cache: [*mut u8; FAT32_CACHE_MAX],
    pub cacheCurr: usize,
}

//
// Externals
//

extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut c_void);
    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);
    fn memset(dst: *mut c_void, val: i32, size: usize);

    fn getDiskBytes(buf: *mut u8, lba: u32, sectors: u32);
}

//
// FAT cache lookup
//

#[no_mangle]
pub unsafe extern "C" fn fat32FATcacheLookup(
    fat: *mut FAT32,
    offset: u32,
) -> u32 {
    for i in 0..FAT32_CACHE_MAX {
        if (*fat).cacheBase[i] == offset {
            return i as u32;
        }
    }

    FAT32_CACHE_BAD
}

//
// FAT cache add
//

#[no_mangle]
pub unsafe extern "C" fn fat32FATcacheAdd(
    fat: *mut FAT32,
    offset: u32,
    bytes: *mut u8,
) {
    if (*fat).cacheCurr >= FAT32_CACHE_MAX {
        (*fat).cacheCurr = 0;
    }

    let idx = (*fat).cacheCurr;
    (*fat).cacheBase[idx] = offset;

    memcpy(
        (*fat).cache[idx] as *mut c_void,
        bytes as *const c_void,
        SECTOR_SIZE,
    );

    (*fat).cacheCurr += 1;
}

//
// FAT sector fetch (with cache)
//

#[no_mangle]
pub unsafe extern "C" fn fat32FATfetch(
    fat: *mut FAT32,
    offsetSector: u32,
    bytes: *mut u8,
) {
    let cache_res = fat32FATcacheLookup(fat, offsetSector);

    if cache_res != FAT32_CACHE_BAD {
        memcpy(
            bytes as *mut c_void,
            (*fat).cache[cache_res as usize] as *const c_void,
            SECTOR_SIZE,
        );
        return;
    }

    getDiskBytes(bytes, offsetSector, 1);
    fat32FATcacheAdd(fat, offsetSector, bytes);
}

//
// FAT chain traversal
//

#[no_mangle]
pub unsafe extern "C" fn fat32FATtraverse(
    fat: *mut FAT32,
    offset: u32,
) -> u32 {
    let bytes_per_cluster = SECTOR_SIZE;

    // FAT32: 4 bytes per entry
    let offset_fat = offset * 4;
    let offset_sector = (*fat).offsetFats + (offset_fat / bytes_per_cluster as u32);
    let offset_entry = (offset_fat % bytes_per_cluster as u32) as usize;

    let bytes = malloc(bytes_per_cluster);
    fat32FATfetch(fat, offset_sector, bytes);

    let ret_location = bytes.add(offset_entry) as *const u32;
    let mut ret = *ret_location & 0x0FFFFFFF;

    free(bytes as *mut c_void);

    // End of chain
    if ret >= 0x0FFFFFF8 {
        return 0;
    }

    // Bad cluster
    if ret == 0x0FFFFFF7 {
        return 0;
    }

    ret
}

//
// FAT chain fetch (+1 for starting cluster)
//

#[no_mangle]
pub unsafe extern "C" fn fat32FATchain(
    fat: *mut FAT32,
    offsetStart: u32,
    amount: u32,
) -> *mut u32 {
    let count = (amount + 1) as usize;
    let ret = malloc(count * core::mem::size_of::<u32>()) as *mut u32;

    memset(
        ret as *mut c_void,
        0,
        count * core::mem::size_of::<u32>(),
    );

    *ret = offsetStart;

    for i in 1..count {
        *ret.add(i) = fat32FATtraverse(fat, *ret.add(i - 1));
    }

    ret
}
