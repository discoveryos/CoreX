// Caching information helpers
// Rust translation

#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ffi::c_void;

// --------------------------------
// External types
// --------------------------------

#[repr(C)]
pub struct MountPoint {
    pub blocksCached: usize,
    // other fields not relevant here
}

#[repr(C)]
pub struct LinkedList {
    _private: [u8; 0],
}

// --------------------------------
// External symbols
// --------------------------------

extern "C" {
    static dsMountPoint: LinkedList;

    fn LinkedListTraverse(
        list: *const LinkedList,
        cb: extern "C" fn(data: *mut c_void, ctx: *mut c_void),
        ctx: *mut c_void,
    );
}

// --------------------------------
// Callback
// --------------------------------

#[no_mangle]
pub extern "C" fn cachingInfoCb(data: *mut c_void, ctx: *mut c_void) {
    unsafe {
        let fs = data as *mut MountPoint;
        let acc = ctx as *mut usize;
        *acc += (*fs).blocksCached;
    }
}

// --------------------------------
// Public API
// --------------------------------

#[no_mangle]
pub extern "C" fn cachingInfoBlocks() -> usize {
    let mut ret: usize = 0;

    unsafe {
        LinkedListTraverse(
            &dsMountPoint as *const _,
            cachingInfoCb,
            &mut ret as *mut _ as *mut c_void,
        );
    }

    ret
}
