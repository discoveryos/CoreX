#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::{mem, ptr};

type size_t = usize;
type uint64_t = u64;
type uint8_t = u8;

/* ================= RESULT ENUM ================= */

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DENTS_RES {
    DENTS_SUCCESS = 0,
    DENTS_RETURN = 1,
    DENTS_NO_SPACE = 2,
}

/* ================= linux_dirent64 ================= */

#[repr(C)]
pub struct linux_dirent64 {
    pub d_ino: uint64_t,
    pub d_off: int64_t,
    pub d_reclen: u16,
    pub d_type: uint8_t,
    pub d_name: [u8; 0], // flexible array member
}

/* ================= KERNEL EXTERNS ================= */

extern "C" {
    fn memcpy(dst: *mut u8, src: *const u8, n: size_t);
    fn rand() -> i32;
}

/* ================= dentsAdd ================= */

pub unsafe fn dentsAdd(
    _buffStart: *mut core::ffi::c_void,
    dirp: &mut *mut linux_dirent64,
    allocatedlimit: &mut size_t,
    hardlimit: u32,
    filename: *const u8,
    filenameLength: size_t,
    inode: size_t,
    dtype: uint8_t,
) -> DENTS_RES {
    // Same formula as C: 23 + filenameLength + 1
    let reclen = 23 + filenameLength + 1;

    if *allocatedlimit + reclen + 2 > hardlimit as usize {
        if *allocatedlimit != 0 {
            return DENTS_RES::DENTS_RETURN;
        } else {
            return DENTS_RES::DENTS_NO_SPACE;
        }
    }

    let entry = &mut **dirp;

    entry.d_reclen = reclen as u16;
    entry.d_ino = inode as u64;
    entry.d_off = rand() as i64; // xd (preserved 😄)
    entry.d_type = dtype;

    // Copy filename
    let name_ptr = entry.d_name.as_ptr() as *mut u8;
    memcpy(name_ptr, filename, filenameLength);
    *name_ptr.add(filenameLength) = 0;

    // Advance counters
    *allocatedlimit += reclen;
    *dirp = (*dirp as usize + reclen) as *mut linux_dirent64;

    DENTS_RES::DENTS_SUCCESS
}
