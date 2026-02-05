#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ptr;

type size_t = usize;
type uint8_t = u8;

/* ================= MACROS ================= */

#[inline(always)]
fn MIN(a: size_t, b: size_t) -> size_t {
    if a < b { a } else { b }
}

#[inline(always)]
fn CIRC_READABLE(write: size_t, read: size_t, size: size_t) -> size_t {
    if write >= read {
        write - read
    } else {
        size - (read - write)
    }
}

#[inline(always)]
fn CIRC_WRITABLE(write: size_t, read: size_t, size: size_t) -> size_t {
    size - CIRC_READABLE(write, read, size) - 1
}

/* ================= STRUCTS ================= */

#[repr(C)]
pub struct Spinlock {
    _dummy: u32,
}

#[repr(C)]
pub struct Circular {
    pub buff: *mut uint8_t,
    pub buffSize: size_t,
    pub readPtr: size_t,
    pub writePtr: size_t,
    pub LOCK_CIRC: Spinlock,
}

#[repr(C)]
pub struct CircularInt {
    pub buff: *mut uint8_t,
    pub buffSize: size_t,
    pub readPtr: u64,
    pub writePtr: u64,
    pub LOCK_READ: Spinlock,
}

/* ================= EXTERNS ================= */

extern "C" {
    fn malloc(size: size_t) -> *mut uint8_t;
    fn free(ptr: *mut uint8_t);
    fn memset(dst: *mut uint8_t, val: i32, size: size_t);
    fn memcpy(dst: *mut uint8_t, src: *const uint8_t, size: size_t);

    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    fn atomicRead64(ptr: *const u64) -> size_t;
    fn atomicWrite64(ptr: *mut u64, val: size_t);

    fn assert(cond: bool);
    fn checkInterrupts() -> bool;
}

/* ================= Circular (locked) ================= */

pub unsafe fn CircularAllocate(circ: *mut Circular, size: size_t) {
    memset(circ as *mut uint8_t, 0, core::mem::size_of::<Circular>());
    (*circ).buffSize = size;
    (*circ).buff = malloc(size);
}

pub unsafe fn CircularRead(
    circ: *mut Circular,
    buff: *mut uint8_t,
    length: size_t,
) -> size_t {
    assert(length != 0);

    spinlockAcquire(&mut (*circ).LOCK_CIRC);

    let write = (*circ).writePtr;
    let read = (*circ).readPtr;

    if write == read {
        spinlockRelease(&mut (*circ).LOCK_CIRC);
        return 0;
    }

    let toCopy = MIN(
        CIRC_READABLE(write, read, (*circ).buffSize),
        length,
    );

    let first = MIN(toCopy, (*circ).buffSize - read);
    memcpy(buff, (*circ).buff.add(read), first);

    let second = toCopy - first;
    if second != 0 {
        memcpy(buff.add(first), (*circ).buff, second);
    }

    (*circ).readPtr = (read + toCopy) % (*circ).buffSize;

    assert(toCopy > 0);
    spinlockRelease(&mut (*circ).LOCK_CIRC);

    toCopy
}

pub unsafe fn CircularReadPoll(circ: *mut Circular) -> size_t {
    spinlockAcquire(&mut (*circ).LOCK_CIRC);
    let ret = CIRC_READABLE(
        (*circ).writePtr,
        (*circ).readPtr,
        (*circ).buffSize,
    );
    spinlockRelease(&mut (*circ).LOCK_CIRC);
    ret
}

pub unsafe fn CircularWritePoll(circ: *mut Circular) -> size_t {
    spinlockAcquire(&mut (*circ).LOCK_CIRC);
    let ret = CIRC_WRITABLE(
        (*circ).writePtr,
        (*circ).readPtr,
        (*circ).buffSize,
    );
    spinlockRelease(&mut (*circ).LOCK_CIRC);
    ret
}

pub unsafe fn CircularWrite(
    circ: *mut Circular,
    buff: *const uint8_t,
    length: size_t,
) -> size_t {
    assert(length != 0);

    spinlockAcquire(&mut (*circ).LOCK_CIRC);

    let write = (*circ).writePtr;
    let read = (*circ).readPtr;
    let writable = CIRC_WRITABLE(write, read, (*circ).buffSize);

    if length > writable {
        spinlockRelease(&mut (*circ).LOCK_CIRC);
        return 0;
    }

    let first = MIN(length, (*circ).buffSize - write);
    memcpy((*circ).buff.add(write), buff, first);

    let second = length - first;
    if second != 0 {
        memcpy((*circ).buff, buff.add(first), second);
    }

    (*circ).writePtr = (write + length) % (*circ).buffSize;
    spinlockRelease(&mut (*circ).LOCK_CIRC);

    length
}

pub unsafe fn CircularFree(circ: *mut Circular) {
    spinlockAcquire(&mut (*circ).LOCK_CIRC);
    free((*circ).buff);
}

/* ================= CircularInt (IRQ-friendly) ================= */

pub unsafe fn CircularIntAllocate(circ: *mut CircularInt, size: size_t) {
    memset(circ as *mut uint8_t, 0, core::mem::size_of::<CircularInt>());
    (*circ).buffSize = size;
    (*circ).buff = malloc(size);
}

pub unsafe fn CircularIntRead(
    circ: *mut CircularInt,
    buff: *mut uint8_t,
    length: size_t,
) -> size_t {
    assert(length != 0);

    spinlockAcquire(&mut (*circ).LOCK_READ);

    let mut write = atomicRead64(&(*circ).writePtr);
    let mut read = atomicRead64(&(*circ).readPtr);

    if write == read {
        spinlockRelease(&mut (*circ).LOCK_READ);
        return 0;
    }

    let toCopy = MIN(
        CIRC_READABLE(write, read, (*circ).buffSize),
        length,
    );

    for i in 0..toCopy {
        *buff.add(i) = *(*circ).buff.add(read);
        read = (read + 1) % (*circ).buffSize;
    }

    assert(toCopy > 0);
    atomicWrite64(&mut (*circ).readPtr, read);

    spinlockRelease(&mut (*circ).LOCK_READ);
    toCopy
}

pub unsafe fn CircularIntReadPoll(circ: *mut CircularInt) -> size_t {
    spinlockAcquire(&mut (*circ).LOCK_READ);
    let ret = CIRC_READABLE(
        atomicRead64(&(*circ).writePtr),
        atomicRead64(&(*circ).readPtr),
        (*circ).buffSize,
    );
    spinlockRelease(&mut (*circ).LOCK_READ);
    ret
}

pub unsafe fn CircularIntWrite(
    circ: *mut CircularInt,
    buff: *const uint8_t,
    length: size_t,
) -> size_t {
    assert(!checkInterrupts());
    assert(length != 0);

    let mut write = atomicRead64(&(*circ).writePtr);
    let read = atomicRead64(&(*circ).readPtr);

    let writable = CIRC_WRITABLE(write, read, (*circ).buffSize);
    if length > writable {
        return 0;
    }

    for i in 0..length {
        *(*circ).buff.add(write) = *buff.add(i);
        write = (write + 1) % (*circ).buffSize;
    }

    atomicWrite64(&mut (*circ).writePtr, write);
    length
}
