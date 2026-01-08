#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

extern crate alloc;

use alloc::alloc::{alloc, dealloc};
use core::ffi::c_void;
use core::mem::{size_of, zeroed};
use core::ptr::{copy_nonoverlapping};

//
// Constants
//

const SECTOR_SIZE: usize = 512;

//
// Structs
//

#[repr(C)]
pub struct Ext2 {
    pub inodeSize: usize,
    pub blockSize: usize,
    pub blockGroups: usize,

    pub bgdts: *mut Ext2Bgdt,
    pub superblock: Ext2Superblock,

    pub WLOCKS_INODE: *mut spinlock_cnt_t,
    pub LOCK_BGDT_WRITE: spinlock_t,
    pub LOCK_SUPERBLOCK_WRITE: spinlock_t,
}

#[repr(C)]
pub struct Ext2Bgdt {
    pub inode_table: u32,
    pub inode_bitmap: u32,
    pub free_inodes: u32,
}

#[repr(C)]
pub struct Ext2Superblock {
    pub free_inodes: u32,
    pub inodes_per_group: u32,
    pub extended: Ext2SuperblockExt,
}

#[repr(C)]
pub struct Ext2SuperblockExt {
    pub first_inode: u32,
}

#[repr(C)]
pub struct Ext2Inode {
    _data: [u8; 0],
}

#[repr(C)]
pub struct spinlock_t {
    _unused: u8,
}

#[repr(C)]
pub struct spinlock_cnt_t {
    _unused: u8,
}

//
// Extern helpers
//

extern "C" {
    fn spinlockAcquire(lock: *mut spinlock_t);
    fn spinlockRelease(lock: *mut spinlock_t);

    fn spinlockCntReadAcquire(lock: *mut spinlock_cnt_t);
    fn spinlockCntReadRelease(lock: *mut spinlock_cnt_t);
    fn spinlockCntWriteAcquire(lock: *mut spinlock_cnt_t);
    fn spinlockCntWriteRelease(lock: *mut spinlock_cnt_t);

    fn getDiskBytes(buf: *mut u8, lba: usize, sectors: usize);
    fn setDiskBytes(buf: *mut u8, lba: usize, sectors: usize);

    fn ext2BgdtPushM(ext2: *mut Ext2);
    fn ext2SuperblockPushM(ext2: *mut Ext2);

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;
}

//
// Helpers
//

#[inline]
fn DivRoundUp(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

#[inline]
fn BLOCK_TO_LBA(_ext2: *mut Ext2, _bg: u32, block: u32) -> usize {
    block as usize
}

#[inline]
fn INODE_TO_BLOCK_GROUP(ext2: *mut Ext2, inode: usize) -> u32 {
    ((inode - 1) / (*ext2).superblock.inodes_per_group as usize) as u32
}

#[inline]
fn INODE_TO_INDEX(ext2: *mut Ext2, inode: usize) -> u32 {
    ((inode - 1) % (*ext2).superblock.inodes_per_group as usize) as u32
}

//
// ext2InodeFetch
//

#[no_mangle]
pub unsafe extern "C" fn ext2InodeFetch(ext2: *mut Ext2, inode: usize) -> *mut Ext2Inode {
    let group = INODE_TO_BLOCK_GROUP(ext2, inode);
    let index = INODE_TO_INDEX(ext2, inode);

    spinlockCntReadAcquire((*ext2).WLOCKS_INODE.add(group as usize));

    let leftovers = index as usize * (*ext2).inodeSize;
    let leftovers_lba = leftovers / SECTOR_SIZE;
    let leftovers_rem = leftovers % SECTOR_SIZE;

    let len = DivRoundUp((*ext2).inodeSize * 4, SECTOR_SIZE) * SECTOR_SIZE;
    let lba = BLOCK_TO_LBA(
        ext2,
        0,
        (*(*ext2).bgdts.add(group as usize)).inode_table,
    ) + leftovers_lba;

    let buf = alloc(core::alloc::Layout::from_size_align(len, 1).unwrap());
    getDiskBytes(buf, lba, len / SECTOR_SIZE);

    let tmp = buf.add(leftovers_rem) as *mut Ext2Inode;

    let ret = alloc(core::alloc::Layout::from_size_align((*ext2).inodeSize, 1).unwrap())
        as *mut Ext2Inode;
    copy_nonoverlapping(tmp as *const u8, ret as *mut u8, (*ext2).inodeSize);

    dealloc(buf, core::alloc::Layout::from_size_align(len, 1).unwrap());
    spinlockCntReadRelease((*ext2).WLOCKS_INODE.add(group as usize));

    ret
}

//
// ext2InodeModifyM
// IMPORTANT: caller must already hold the inode lock
//

#[no_mangle]
pub unsafe extern "C" fn ext2InodeModifyM(
    ext2: *mut Ext2,
    inode: usize,
    target: *mut Ext2Inode,
) {
    let group = INODE_TO_BLOCK_GROUP(ext2, inode);
    let index = INODE_TO_INDEX(ext2, inode);

    spinlockCntWriteAcquire((*ext2).WLOCKS_INODE.add(group as usize));

    let leftovers = index as usize * (*ext2).inodeSize;
    let leftovers_lba = leftovers / SECTOR_SIZE;
    let leftovers_rem = leftovers % SECTOR_SIZE;

    let len = DivRoundUp((*ext2).inodeSize * 4, SECTOR_SIZE) * SECTOR_SIZE;
    let lba = BLOCK_TO_LBA(
        ext2,
        0,
        (*(*ext2).bgdts.add(group as usize)).inode_table,
    ) + leftovers_lba;

    let buf = alloc(core::alloc::Layout::from_size_align(len, 1).unwrap());
    getDiskBytes(buf, lba, len / SECTOR_SIZE);

    let tmp = buf.add(leftovers_rem) as *mut Ext2Inode;
    copy_nonoverlapping(target as *const u8, tmp as *mut u8, size_of::<Ext2Inode>());

    setDiskBytes(buf, lba, len / SECTOR_SIZE);
    dealloc(buf, core::alloc::Layout::from_size_align(len, 1).unwrap());

    spinlockCntWriteRelease((*ext2).WLOCKS_INODE.add(group as usize));
}

//
// ext2InodeDelete
//

#[no_mangle]
pub unsafe extern "C" fn ext2InodeDelete(ext2: *mut Ext2, inode: usize) {
    let group = INODE_TO_BLOCK_GROUP(ext2, inode);
    let index = INODE_TO_INDEX(ext2, inode);

    spinlockCntWriteAcquire((*ext2).WLOCKS_INODE.add(group as usize));

    let where_ = index / 8;
    let remainder = index % 8;

    let lba = BLOCK_TO_LBA(
        ext2,
        0,
        (*(*ext2).bgdts.add(group as usize)).inode_bitmap,
    );

    let buf = alloc(core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap());
    getDiskBytes(buf, lba, (*ext2).blockSize / SECTOR_SIZE);

    let byte = buf.add(where_ as usize);
    *byte &= !(1 << remainder);

    setDiskBytes(buf, lba, (*ext2).blockSize / SECTOR_SIZE);
    dealloc(buf, core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap());

    spinlockAcquire(&mut (*ext2).LOCK_BGDT_WRITE);
    (*(*ext2).bgdts.add(group as usize)).free_inodes += 1;
    ext2BgdtPushM(ext2);
    spinlockRelease(&mut (*ext2).LOCK_BGDT_WRITE);

    spinlockAcquire(&mut (*ext2).LOCK_SUPERBLOCK_WRITE);
    (*ext2).superblock.free_inodes += 1;
    ext2SuperblockPushM(ext2);
    spinlockRelease(&mut (*ext2).LOCK_SUPERBLOCK_WRITE);

    spinlockCntWriteRelease((*ext2).WLOCKS_INODE.add(group as usize));
}

//
// ext2InodeFind
//

#[no_mangle]
pub unsafe extern "C" fn ext2InodeFind(ext2: *mut Ext2, groupSuggestion: i32) -> u32 {
    if (*ext2).superblock.free_inodes < 1 {
        debugf(b"[ext2] FATAL! Couldn't find a single inode! Drive is full!\n\0".as_ptr());
        panic();
    }

    let suggested = ext2InodeFindL(ext2, groupSuggestion);
    if suggested != 0 {
        return suggested;
    }

    for i in 0..(*ext2).blockGroups {
        if i as i32 == groupSuggestion {
            continue;
        }
        let ret = ext2InodeFindL(ext2, i as i32);
        if ret != 0 {
            return ret;
        }
    }

    debugf(b"[ext2] FATAL! Couldn't find a single inode! Drive is full!\n\0".as_ptr());
    panic();
}

//
// ext2InodeFindL
//

#[no_mangle]
pub unsafe extern "C" fn ext2InodeFindL(ext2: *mut Ext2, group: i32) -> u32 {
    let bgdt = &mut *(*ext2).bgdts.add(group as usize);
    if bgdt.free_inodes < 1 {
        return 0;
    }

    spinlockCntWriteAcquire((*ext2).WLOCKS_INODE.add(group as usize));

    let buff = alloc(core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap());
    getDiskBytes(
        buff,
        BLOCK_TO_LBA(ext2, 0, bgdt.inode_bitmap),
        (*ext2).blockSize / SECTOR_SIZE,
    );

    let mut ret: u32 = 0;

    let mut first_div = 0usize;
    let mut first_rem = 0usize;

    if group == 0 {
        first_div = (*ext2).superblock.extended.first_inode as usize / 8;
        first_rem = (*ext2).superblock.extended.first_inode as usize % 8;
    }

    for i in first_div..(*ext2).blockSize {
        if *buff.add(i) == 0xff {
            continue;
        }
        for j in if i == first_div { first_rem } else { 0 }..8 {
            if (*buff.add(i) & (1 << j)) == 0 {
                ret = (i * 8 + j) as u32;
                break;
            }
        }
        if ret != 0 {
            break;
        }
    }

    if ret != 0 {
        let where_ = ret / 8;
        let rem = ret % 8;
        *buff.add(where_ as usize) |= 1 << rem;

        setDiskBytes(
            buff,
            BLOCK_TO_LBA(ext2, 0, bgdt.inode_bitmap),
            (*ext2).blockSize / SECTOR_SIZE,
        );

        spinlockAcquire(&mut (*ext2).LOCK_BGDT_WRITE);
        bgdt.free_inodes -= 1;
        ext2BgdtPushM(ext2);
        spinlockRelease(&mut (*ext2).LOCK_BGDT_WRITE);

        spinlockAcquire(&mut (*ext2).LOCK_SUPERBLOCK_WRITE);
        (*ext2).superblock.free_inodes -= 1;
        ext2SuperblockPushM(ext2);
        spinlockRelease(&mut (*ext2).LOCK_SUPERBLOCK_WRITE);
    }

    dealloc(buff, core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap());
    spinlockCntWriteRelease((*ext2).WLOCKS_INODE.add(group as usize));

    if ret != 0 {
        (group as u32 * (*ext2).superblock.inodes_per_group) + ret + 1
    } else {
        0
    }
}
