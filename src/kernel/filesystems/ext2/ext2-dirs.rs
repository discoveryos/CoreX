#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

extern crate alloc;

use alloc::alloc::{alloc, dealloc};
use core::ffi::c_void;
use core::mem::{size_of, zeroed};
use core::ptr::{copy_nonoverlapping, null_mut};

//
// Constants
//

const SECTOR_SIZE: u32 = 512;

const ENOTDIR: usize = 20;
const EINVAL: usize = 22;

const EXT2_S_IFDIR: u32 = 0x4000;

const CDT_DIR: u8 = 4;
const CDT_LNK: u8 = 10;
const CDT_REG: u8 = 8;

#[inline]
fn ERR(e: usize) -> usize {
    (-(e as isize)) as usize
}

//
// Structs
//

#[repr(C)]
pub struct Ext2 {
    pub blockSize: usize,
    pub LOCK_DIRALLOC: spinlock_t,
}

#[repr(C)]
pub struct spinlock_t {
    _unused: u8,
}

#[repr(C)]
pub struct Ext2Inode {
    pub permission: u32,
    pub size: u32,
    pub num_sectors: u32,
}

#[repr(C)]
pub struct Ext2Directory {
    pub inode: u32,
    pub size: u16,
    pub filenameLength: u8,
    pub type_: u8,
    pub filename: [u8; 0],
}

#[repr(C)]
pub struct Ext2LookupControl {
    _unused: u8,
}

#[repr(C)]
pub struct OpenFile {
    pub mountPoint: *mut MountPoint,
    pub dir: *mut c_void,
}

#[repr(C)]
pub struct MountPoint {
    pub fsInfo: *mut c_void,
}

#[repr(C)]
pub struct Ext2OpenFd {
    pub inode: Ext2Inode,
    pub inodeNum: u32,
    pub lookup: Ext2LookupControl,
    pub ptr: usize,
}

#[repr(C)]
pub struct linux_dirent64 {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
    pub d_name: [u8; 0],
}

//
// Enums
//

#[repr(C)]
pub enum DENTS_RES {
    DENTS_OK,
    DENTS_RETURN,
    DENTS_NO_SPACE,
}

//
// External helpers
//

extern "C" {
    fn spinlockAcquire(lock: *mut spinlock_t);
    fn spinlockRelease(lock: *mut spinlock_t);

    fn ext2BlockFetchInit(fs: *mut Ext2, ctrl: *mut Ext2LookupControl);
    fn ext2BlockFetchCleanup(ctrl: *mut Ext2LookupControl);

    fn ext2BlockFetch(
        fs: *mut Ext2,
        inode: *mut Ext2Inode,
        inode_num: u32,
        ctrl: *mut Ext2LookupControl,
        block_num: usize,
    ) -> usize;

    fn ext2BlockFind(fs: *mut Ext2, group: u32, count: u32) -> u32;
    fn ext2BlockAssign(
        fs: *mut Ext2,
        inode: *mut Ext2Inode,
        inode_num: u32,
        ctrl: *mut Ext2LookupControl,
        block_num: usize,
        block: u32,
    );

    fn ext2InodeModifyM(fs: *mut Ext2, inode: u32, data: *const Ext2Inode);

    fn getDiskBytes(buf: *mut u8, lba: usize, sectors: usize);
    fn setDiskBytes(buf: *mut u8, lba: usize, sectors: usize);

    fn dentsAdd(
        start: *mut linux_dirent64,
        cur: *mut *mut linux_dirent64,
        used: *mut usize,
        limit: usize,
        name: *const u8,
        namelen: u8,
        inode: u32,
        dtype: u8,
    ) -> DENTS_RES;
}

#[inline]
fn EXT2_PTR(p: *mut c_void) -> *mut Ext2 {
    p as *mut Ext2
}

#[inline]
fn EXT2_DIR_PTR(p: *mut c_void) -> *mut Ext2OpenFd {
    p as *mut Ext2OpenFd
}

#[inline]
fn DivRoundUp(a: u32, b: usize) -> usize {
    ((a as usize) + b - 1) / b
}

#[inline]
fn BLOCK_TO_LBA(_fs: *mut Ext2, _bg: u32, block: usize) -> usize {
    block
}

#[inline]
fn INODE_TO_BLOCK_GROUP(_fs: *mut Ext2, inode: u32) -> u32 {
    inode
}

//
// ext2DirAllocate
//

#[no_mangle]
pub unsafe extern "C" fn ext2DirAllocate(
    ext2: *mut Ext2,
    inodeNum: u32,
    parentDirInode: *mut Ext2Inode,
    filename: *const u8,
    filenameLen: u8,
    type_: u8,
    inode: u32,
) -> bool {
    spinlockAcquire(&mut (*ext2).LOCK_DIRALLOC);

    let entry_len = size_of::<Ext2Directory>() + filenameLen as usize;
    let ino = parentDirInode;

    let layout = core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap();
    let names = alloc(layout);

    let mut control: Ext2LookupControl = zeroed();
    let mut block_num = 0usize;
    let mut ret = false;

    ext2BlockFetchInit(ext2, &mut control);

    let blocks = DivRoundUp((*ino).size, (*ext2).blockSize);
    for _ in 0..blocks {
        let block = ext2BlockFetch(ext2, ino, inodeNum, &mut control, block_num);
        if block == 0 {
            break;
        }
        block_num += 1;

        getDiskBytes(
            names,
            BLOCK_TO_LBA(ext2, 0, block),
            (*ext2).blockSize / SECTOR_SIZE as usize,
        );

        let mut dir = names as *mut Ext2Directory;
        while (dir as usize - names as usize) < (*ext2).blockSize {
            if (*dir).inode != 0
                && (*dir).filenameLength == filenameLen
                && core::slice::from_raw_parts((*dir).filename.as_ptr(), filenameLen as usize)
                    == core::slice::from_raw_parts(filename, filenameLen as usize)
            {
                goto_cleanup!(ret = false);
            }

            let min_old = (size_of::<Ext2Directory>() + (*dir).filenameLength as usize + 3) & !3;
            let min_new = (entry_len + 3) & !3;

            let remainder = (*dir).size as usize - min_old;
            if remainder < min_new {
                dir = (dir as usize + (*dir).size as usize) as *mut _;
                continue;
            }

            (*dir).size = min_old as u16;
            let new = (dir as usize + (*dir).size as usize) as *mut Ext2Directory;

            (*new).size = remainder as u16;
            (*new).type_ = type_;
            (*new).filenameLength = filenameLen;
            (*new).inode = inode;
            copy_nonoverlapping(filename, (*new).filename.as_mut_ptr(), filenameLen as usize);

            setDiskBytes(
                names,
                BLOCK_TO_LBA(ext2, 0, block),
                (*ext2).blockSize / SECTOR_SIZE as usize,
            );

            goto_cleanup!(ret = true);
        }
    }

    let group = INODE_TO_BLOCK_GROUP(ext2, inodeNum);
    let new_block = ext2BlockFind(ext2, group, 1);

    let new = names as *mut Ext2Directory;
    (*new).size = (*ext2).blockSize as u16;
    (*new).type_ = type_;
    (*new).filenameLength = filenameLen;
    (*new).inode = inode;
    copy_nonoverlapping(filename, (*new).filename.as_mut_ptr(), filenameLen as usize);

    setDiskBytes(
        names,
        BLOCK_TO_LBA(ext2, 0, new_block as usize),
        (*ext2).blockSize / SECTOR_SIZE as usize,
    );

    ext2BlockAssign(ext2, ino, inodeNum, &mut control, block_num, new_block);

    (*ino).num_sectors += (*ext2).blockSize as u32 / SECTOR_SIZE;
    (*ino).size += (*ext2).blockSize as u32;

    ext2InodeModifyM(ext2, inodeNum, ino);

    ret = true;

cleanup:
    ext2BlockFetchCleanup(&mut control);
    dealloc(names, layout);
    spinlockRelease(&mut (*ext2).LOCK_DIRALLOC);
    ret
}

//
// ext2DirRemove
//

#[no_mangle]
pub unsafe extern "C" fn ext2DirRemove(
    ext2: *mut Ext2,
    parentDirInode: *mut Ext2Inode,
    parentDirInodeNum: u32,
    filename: *const u8,
    filenameLen: u8,
) -> bool {
    spinlockAcquire(&mut (*ext2).LOCK_DIRALLOC);

    let ino = parentDirInode;
    let layout = core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap();
    let names = alloc(layout);

    let mut control: Ext2LookupControl = zeroed();
    let mut block_num = 0usize;
    let mut ret = false;

    ext2BlockFetchInit(ext2, &mut control);

    let blocks = DivRoundUp((*ino).size, (*ext2).blockSize);
    for _ in 0..blocks {
        let block = ext2BlockFetch(ext2, ino, parentDirInodeNum, &mut control, block_num);
        if block == 0 {
            break;
        }
        block_num += 1;

        getDiskBytes(
            names,
            BLOCK_TO_LBA(ext2, 0, block),
            (*ext2).blockSize / SECTOR_SIZE as usize,
        );

        let mut dir = names as *mut Ext2Directory;
        let mut before: *mut Ext2Directory = null_mut();

        while (dir as usize - names as usize) < (*ext2).blockSize {
            if (*dir).inode != 0
                && (*dir).filenameLength == filenameLen
                && core::slice::from_raw_parts((*dir).filename.as_ptr(), filenameLen as usize)
                    == core::slice::from_raw_parts(filename, filenameLen as usize)
            {
                if before.is_null() {
                    (*dir).inode = 0;
                    (*dir).filenameLength = 0;
                } else {
                    (*before).size += (*dir).size;
                }

                setDiskBytes(
                    names,
                    BLOCK_TO_LBA(ext2, 0, block),
                    (*ext2).blockSize / SECTOR_SIZE as usize,
                );

                ret = true;
            }

            before = dir;
            dir = (dir as usize + (*dir).size as usize) as *mut _;
        }
    }

    ext2BlockFetchCleanup(&mut control);
    dealloc(names, layout);
    spinlockRelease(&mut (*ext2).LOCK_DIRALLOC);
    ret
}

//
// ext2Getdents64
//

#[no_mangle]
pub unsafe extern "C" fn ext2Getdents64(
    file: *mut OpenFile,
    start: *mut linux_dirent64,
    hardlimit: usize,
) -> usize {
    let ext2 = EXT2_PTR((*(*file).mountPoint).fsInfo);
    let edir = EXT2_DIR_PTR((*file).dir);

    if ((*edir).inode.permission & 0xF000) != EXT2_S_IFDIR {
        return ERR(ENOTDIR);
    }

    let mut allocated = 0usize;
    let ino = &mut (*edir).inode;

    let layout = core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap();
    let names = alloc(layout);

    let mut dirp = start;

    let blocks = DivRoundUp(ino.size, (*ext2).blockSize);
    for _ in 0..blocks {
        let block = ext2BlockFetch(
            ext2,
            ino,
            (*edir).inodeNum,
            &mut (*edir).lookup,
            (*edir).ptr / (*ext2).blockSize,
        );
        if block == 0 {
            break;
        }

        getDiskBytes(
            names,
            BLOCK_TO_LBA(ext2, 0, block),
            (*ext2).blockSize / SECTOR_SIZE as usize,
        );

        let mut dir = (names as usize + ((*edir).ptr % (*ext2).blockSize)) as *mut Ext2Directory;

        while (dir as usize - names as usize) < (*ext2).blockSize {
            if (*dir).inode == 0 {
                dir = (dir as usize + (*dir).size as usize) as *mut _;
                continue;
            }

            let dtype = match (*dir).type_ {
                2 => CDT_DIR,
                7 => CDT_LNK,
                _ => CDT_REG,
            };

            match dentsAdd(
                start,
                &mut dirp,
                &mut allocated,
                hardlimit,
                (*dir).filename.as_ptr(),
                (*dir).filenameLength,
                (*dir).inode,
                dtype,
            ) {
                DENTS_RES::DENTS_NO_SPACE => {
                    allocated = ERR(EINVAL);
                    goto_cleanup!(());
                }
                DENTS_RES::DENTS_RETURN => goto_cleanup!(()),
                _ => {}
            }

            (*edir).ptr += (*dir).size as usize;
            dir = (dir as usize + (*dir).size as usize) as *mut _;
        }

        let rem = (*edir).ptr % (*ext2).blockSize;
        if rem != 0 {
            (*edir).ptr += (*ext2).blockSize - rem;
        }
    }

cleanup:
    dealloc(names, layout);
    allocated
}
