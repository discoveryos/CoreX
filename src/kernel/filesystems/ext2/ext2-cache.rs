#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ptr::{null_mut};

//
// ====== FFI & low-level helpers ======
//

extern "C" {
    fn spinlockCntWriteAcquire(lock: *mut Spinlock);
    fn spinlockCntWriteRelease(lock: *mut Spinlock);

    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    fn VirtualFree(ptr: *mut u8, pages: usize);
    fn calloc(size: usize, count: usize) -> *mut u8;
    fn free(ptr: *mut u8);
}

const BLOCK_SIZE: usize = 4096;

#[inline]
fn DivRoundUp(x: usize, y: usize) -> usize {
    (x + y - 1) / y
}

//
// ====== Core data structures ======
//

#[repr(C)]
pub struct Spinlock {
    _dummy: u32,
}

#[repr(C)]
pub struct MountPoint {
    pub fsInfo: *mut core::ffi::c_void,
    pub blocksCached: usize,
}

#[repr(C)]
pub struct Ext2 {
    pub blockSize: usize,
    pub LOCK_OBJECT: Spinlock,
    pub firstObject: *mut Ext2FoundObject,
}

#[repr(C)]
pub struct Ext2CacheObject {
    pub blockIndex: usize,
    pub blocks: usize,
    pub buff: *mut u8,

    pub prev: *mut Ext2CacheObject,
    pub next: *mut Ext2CacheObject,
}

#[repr(C)]
pub struct Ext2FoundObject {
    pub WLOCK_CACHE: Spinlock,
    pub firstCacheObj: *mut Ext2CacheObject,

    pub prev: *mut Ext2FoundObject,
    pub next: *mut Ext2FoundObject,
}

#[repr(C)]
pub struct Ext2OpenFd {
    pub globalObject: *mut Ext2FoundObject,
}

#[inline]
unsafe fn EXT2_PTR(ptr: *mut core::ffi::c_void) -> *mut Ext2 {
    ptr as *mut Ext2
}

//
// ====== ext2CacheAddSecurely ======
//

pub unsafe fn ext2CacheAddSecurely(
    mnt: *mut MountPoint,
    global: *mut Ext2FoundObject,
    buff: *mut u8,
    blockIndex: usize,
    blocks: usize,
) {
    let ext2 = EXT2_PTR((*mnt).fsInfo);

    spinlockCntWriteAcquire(&mut (*global).WLOCK_CACHE);

    // find anything as a start that is close
    let mut cacheObj = (*global).firstCacheObj;
    let mut lastCaught = (*global).firstCacheObj;

    while !cacheObj.is_null() {
        if (*cacheObj).blockIndex >= blockIndex
            && (*cacheObj).blockIndex < (blockIndex + blocks)
        {
            break;
        }
        lastCaught = cacheObj;
        cacheObj = (*cacheObj).next;
    }

    if cacheObj.is_null() {
        // no overlap
        let mut browse = (*global).firstCacheObj;
        let mut allAreSmaller = false;

        while !browse.is_null() {
            if (*browse).blockIndex < blockIndex {
                allAreSmaller = true;
                browse = (*browse).next;
            } else if (*browse).blockIndex == blockIndex {
                panic!("blockIndex already cached");
            } else {
                browse = (*browse).prev;
                break;
            }
        }

        let target = calloc(core::mem::size_of::<Ext2CacheObject>(), 1)
            as *mut Ext2CacheObject;

        (*mnt).blocksCached += blocks;

        (*target).blockIndex = blockIndex;
        (*target).blocks = blocks;
        (*target).buff = buff;
        (*target).prev = null_mut();
        (*target).next = null_mut();

        if (*global).firstCacheObj.is_null() {
            (*global).firstCacheObj = target;
        } else if !browse.is_null() {
            let next = (*browse).next;
            (*browse).next = target;
            (*target).prev = browse;

            (*target).next = next;
            if !next.is_null() {
                (*next).prev = target;
            }
        } else if allAreSmaller && !lastCaught.is_null() {
            (*lastCaught).next = target;
            (*target).prev = lastCaught;
        } else if !allAreSmaller {
            (*target).next = (*global).firstCacheObj;
            (*global).firstCacheObj = target;
            if !(*target).next.is_null() {
                (*(*target).next).prev = target;
            }
        } else {
            panic!("invalid cache insert state");
        }
    } else {
        // overlapping cache region exists
        let mut browse = cacheObj;

        while !browse.is_null() {
            if !((*browse).blockIndex >= blockIndex
                && (*browse).blockIndex < (blockIndex + blocks))
            {
                break;
            }

            VirtualFree(
                (*browse).buff,
                DivRoundUp(
                    ((*browse).blocks + 1) * (*ext2).blockSize,
                    BLOCK_SIZE,
                ),
            );

            (*mnt).blocksCached -= (*browse).blocks;

            let next = (*browse).next;

            if (*browse).prev.is_null() {
                (*global).firstCacheObj = next;
                if !next.is_null() {
                    (*next).prev = null_mut();
                }
            } else {
                let prev = (*browse).prev;
                (*prev).next = next;
                if !next.is_null() {
                    (*next).prev = prev;
                }
            }

            free(browse as *mut u8);
            browse = next;
        }

        spinlockCntWriteRelease(&mut (*global).WLOCK_CACHE);
        ext2CacheAddSecurely(mnt, global, buff, blockIndex, blocks);
        return;
    }

    spinlockCntWriteRelease(&mut (*global).WLOCK_CACHE);
}

//
// ====== ext2CachePush ======
//

pub unsafe fn ext2CachePush(ext2: *mut Ext2, fd: *mut Ext2OpenFd) {
    if (*ext2).firstObject == (*fd).globalObject {
        return;
    }

    spinlockAcquire(&mut (*ext2).LOCK_OBJECT);

    let beforeFd = (*(*fd).globalObject).prev;
    let afterFd = (*(*fd).globalObject).next;

    (*(*fd).globalObject).next = (*ext2).firstObject;
    (*(*fd).globalObject).prev = null_mut();

    if !beforeFd.is_null() {
        (*beforeFd).next = afterFd;
    }
    if !afterFd.is_null() {
        (*afterFd).prev = beforeFd;
    }

    (*ext2).firstObject = (*fd).globalObject;

    if !(*ext2).firstObject.is_null()
        && !(*(*ext2).firstObject).next.is_null()
    {
        (*(*(*ext2).firstObject).next).prev = (*ext2).firstObject;
    }

    spinlockRelease(&mut (*ext2).LOCK_OBJECT);
}
