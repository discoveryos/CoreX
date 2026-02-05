#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::{ptr, mem};

type size_t = usize;
type uint32_t = u32;
type bool_t = bool;

/* ================= CONSTANTS ================= */

pub const LL_SIGNATURE_1: u32 = 0x4C4C3131; // example: "LL11"
pub const LL_SIGNATURE_2: u32 = 0x4C4C3232; // example: "LL22"

/* ================= STRUCTS ================= */

/*
 * IMPORTANT:
 * The first field of every linked-list object MUST be `next`
 */
#[repr(C)]
pub struct LLheader {
    pub next: *mut LLheader,
}

#[repr(C)]
pub struct LLcontrol {
    pub signature1: u32,
    pub signature2: u32,
    pub structSize: u32,
    pub firstObject: *mut LLheader,
}

/* ================= KERNEL EXTERNS ================= */

extern "C" {
    fn malloc(size: size_t) -> *mut u8;
    fn free(ptr: *mut u8);
    fn memset(dst: *mut u8, val: i32, size: size_t);
    fn assert(cond: bool);
}

/* ================= INTERNAL ================= */

#[inline(always)]
unsafe fn LinkedListNormal(ll: *mut LLcontrol, structSize: u32) {
    assert((*ll).signature1 == LL_SIGNATURE_1);
    assert((*ll).signature2 == LL_SIGNATURE_2);
    assert((*ll).structSize == structSize);
}

/* ================= API ================= */

pub unsafe fn LinkedListAllocate(
    ll: *mut LLcontrol,
    structSize: u32,
) -> *mut u8 {
    LinkedListNormal(ll, structSize);

    let first_ptr = &mut (*ll).firstObject as *mut *mut LLheader;

    let target = malloc(structSize as usize) as *mut LLheader;
    memset(target as *mut u8, 0, structSize as usize);

    let mut curr = *first_ptr;
    loop {
        if curr.is_null() {
            *first_ptr = target;
            break;
        }
        if (*curr).next.is_null() {
            (*curr).next = target;
            break;
        }
        curr = (*curr).next;
    }

    (*target).next = ptr::null_mut();
    target as *mut u8
}

pub unsafe fn LinkedListUnregister(
    ll: *mut LLcontrol,
    structSize: u32,
    LLtarget: *const u8,
) -> bool_t {
    LinkedListNormal(ll, structSize);

    let first_ptr = &mut (*ll).firstObject as *mut *mut LLheader;
    let first_copy = *first_ptr;

    let mut curr = *first_ptr;
    while !curr.is_null() {
        if !(*curr).next.is_null() && (*curr).next as *const u8 == LLtarget {
            break;
        }
        curr = (*curr).next;
    }

    if first_copy as *const u8 == LLtarget {
        *first_ptr = (*first_copy).next;
        return true;
    } else if curr.is_null() {
        return false;
    }

    let target = (*curr).next;
    (*curr).next = (*target).next;

    true
}

pub unsafe fn LinkedListRemove(
    ll: *mut LLcontrol,
    structSize: u32,
    LLtarget: *mut u8,
) -> bool_t {
    LinkedListNormal(ll, structSize);
    let res = LinkedListUnregister(ll, structSize, LLtarget);
    free(LLtarget);
    res
}

pub unsafe fn LinkedListPushFrontUnsafe(
    ll: *mut LLcontrol,
    LLtarget: *mut u8,
) {
    let first_ptr = &mut (*ll).firstObject as *mut *mut LLheader;

    if (*first_ptr).is_null() {
        *first_ptr = LLtarget as *mut LLheader;
        assert((*(LLtarget as *mut LLheader)).next.is_null());
        return;
    }

    let next = *first_ptr;
    *first_ptr = LLtarget as *mut LLheader;
    (*(LLtarget as *mut LLheader)).next = next;
}

pub unsafe fn LinkedListDestroy(
    ll: *mut LLcontrol,
    structSize: u32,
) {
    LinkedListNormal(ll, structSize);

    let first_ptr = &mut (*ll).firstObject as *mut *mut LLheader;
    let mut browse = *first_ptr;

    while !browse.is_null() {
        let next = (*browse).next;
        free(browse as *mut u8);
        browse = next;
    }

    *first_ptr = ptr::null_mut();
}

pub unsafe fn LinkedListInit(
    ll: *mut LLcontrol,
    structSize: u32,
) {
    assert((*ll).signature1 != LL_SIGNATURE_1);
    assert((*ll).signature2 != LL_SIGNATURE_2);

    memset(ll as *mut u8, 0, mem::size_of::<LLcontrol>());

    (*ll).signature1 = LL_SIGNATURE_1;
    (*ll).signature2 = LL_SIGNATURE_2;
    (*ll).structSize = structSize;
}

pub unsafe fn LinkedListTraverse(
    ll: *mut LLcontrol,
    callback: extern "C" fn(data: *mut u8, ctx: *mut u8),
    ctx: *mut u8,
) {
    let mut browse = (*ll).firstObject;
    while !browse.is_null() {
        callback(browse as *mut u8, ctx);
        browse = (*browse).next;
    }
}

pub unsafe fn LinkedListSearch(
    ll: *mut LLcontrol,
    isCorrect: extern "C" fn(data: *mut u8, ctx: *mut u8) -> bool,
    ctx: *mut u8,
) -> *mut u8 {
    let mut browse = (*ll).firstObject;
    while !browse.is_null() {
        if isCorrect(browse as *mut u8, ctx) {
            break;
        }
        browse = (*browse).next;
    }
    browse as *mut u8
}

extern "C" fn LinkedListSearchPtrCb(data: *mut u8, target: *mut u8) -> bool {
    data == target
}

pub unsafe fn LinkedListSearchPtr(
    ll: *mut LLcontrol,
    targetPtr: *mut u8,
) -> *mut u8 {
    LinkedListSearch(ll, LinkedListSearchPtrCb, targetPtr)
}

extern "C" fn LinkedListSearchFirstCb(_data: *mut u8, _ctx: *mut u8) -> bool {
    true
}

pub unsafe fn LinkedListSearchFirst(ll: *mut LLcontrol) -> *mut u8 {
    LinkedListSearch(ll, LinkedListSearchFirstCb, ptr::null_mut())
}
