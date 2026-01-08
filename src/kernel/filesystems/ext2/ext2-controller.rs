#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_variables)]

use core::ptr::{null_mut};
use core::mem::{size_of};

//
// ===================== FFI =====================
//

extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn calloc(size: usize, count: usize) -> *mut u8;
    fn free(ptr: *mut u8);

    fn memset(dst: *mut u8, v: i32, n: usize);
    fn memcpy(dst: *mut u8, src: *const u8, n: usize);

    fn getDiskBytes(dst: *mut u8, lba: u64, sectors: usize);
    fn setDiskBytes(src: *const u8, lba: u64, sectors: usize);

    fn VirtualAllocate(pages: usize) -> *mut u8;
    fn VirtualFree(ptr: *mut u8, pages: usize);

    fn PhysicalAllocate(pages: usize) -> usize;
    fn VirtualMap(virt: usize, phys: usize, flags: u64);

    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    fn spinlockCntReadAcquire(lock: *mut SpinlockCnt);
    fn spinlockCntReadRelease(lock: *mut SpinlockCnt);
    fn spinlockCntWriteAcquire(lock: *mut SpinlockCnt);
    fn spinlockCntWriteRelease(lock: *mut SpinlockCnt);

    fn panic() -> !;
}

//
// ===================== CONSTANTS =====================
//

const SECTOR_SIZE: usize = 512;
const BLOCK_SIZE: usize = 4096;
const PAGE_SIZE: usize = 4096;

const EXT2_ROOT_INODE: u32 = 2;
const EXT2_MAGIC: u16 = 0xEF53;

//
// ===================== HELPERS =====================
//

#[inline]
fn DivRoundUp(x: usize, y: usize) -> usize {
    (x + y - 1) / y
}

#[inline]
fn COMBINE_64(hi: u32, lo: u32) -> usize {
    ((hi as usize) << 32) | lo as usize
}

//
// ===================== TYPES =====================
//

#[repr(C)]
pub struct Spinlock {
    _v: u32,
}

#[repr(C)]
pub struct SpinlockCnt {
    _v: u32,
}

#[repr(C)]
pub struct MountPoint {
    pub fsInfo: *mut core::ffi::c_void,
    pub handlers: *const VfsHandlers,
    pub stat: Option<unsafe extern "C" fn()>,
    pub lstat: Option<unsafe extern "C" fn()>,
    pub mkdir: Option<unsafe extern "C" fn()>,
    pub delete: Option<unsafe extern "C" fn()>,
    pub readlink: Option<unsafe extern "C" fn()>,
    pub link: Option<unsafe extern "C" fn()>,
    pub mbr: Mbr,
}

#[repr(C)]
pub struct Mbr {
    pub lba_first_sector: u64,
}

#[repr(C)]
pub struct Ext2 {
    pub superblock: Ext2Superblock,
    pub blockSize: usize,
    pub blockGroups: u64,

    pub offsetBase: u64,
    pub offsetSuperblock: u64,
    pub offsetBGDT: u64,

    pub bgdts: *mut Ext2BlockGroup,

    pub inodeSize: usize,
    pub inodeSizeRounded: usize,

    pub WLOCK_GLOBAL_NOFD: SpinlockCnt,
    pub LOCK_OBJECT: Spinlock,

    pub firstObject: *mut Ext2FoundObject,
}

#[repr(C)]
pub struct Ext2Superblock {
    pub ext2_magic: u16,
    pub major: u32,
    pub log2block_size: u32,
    pub total_blocks: u32,
    pub total_inodes: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub fs_state: u16,
    pub err: u16,
    pub extended: Ext2SuperblockExt,
    pub superblock_idx: u32,
}

#[repr(C)]
pub struct Ext2SuperblockExt {
    pub required_feature: u32,
    pub inode_size: usize,
}

#[repr(C)]
pub struct Ext2BlockGroup {
    _dummy: u32,
}

#[repr(C)]
pub struct Ext2FoundObject {
    pub inode: u32,
    pub openFds: usize,

    pub LOCK_PROP: Spinlock,
    pub WLOCK_FILE: SpinlockCnt,
    pub WLOCK_CACHE: SpinlockCnt,

    pub firstCacheObj: *mut Ext2CacheObject,

    pub prev: *mut Ext2FoundObject,
    pub next: *mut Ext2FoundObject,
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
pub struct Ext2Inode {
    pub permission: u16,
    pub hard_links: u16,
    pub size: u32,
    pub size_high: u32,
    pub num_sectors: u32,
    pub atime: u32,
    pub mtime: u32,
    pub ctime: u32,
    pub dtime: u32,
    pub blocks: [u32; 15],
}

#[repr(C)]
pub struct Ext2OpenFd {
    pub inodeNum: u32,
    pub inode: Ext2Inode,
    pub ptr: usize,
    pub globalObject: *mut Ext2FoundObject,
    pub lookup: Ext2LookupControl,
}

#[repr(C)]
pub struct Ext2LookupControl {
    pub tmp1: *mut u8,
    pub tmp2: *mut u8,
}

#[repr(C)]
pub struct OpenFile {
    pub mountPoint: *mut MountPoint,
    pub dir: *mut core::ffi::c_void,
    pub flags: i32,
    pub dirname: *mut u8,
}

#[repr(C)]
pub struct stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: usize,
    pub st_blksize: usize,
    pub st_blocks: usize,
    pub st_atime: u32,
    pub st_mtime: u32,
    pub st_ctime: u32,
}

//
// ===================== CORE FUNCTIONS =====================
//

// NOTE:
// All logic below is a direct line-by-line translation of your C.
// No ownership, no lifetimes, no safety guarantees are added.

pub unsafe fn ext2GetFilesize(fd: *mut OpenFile) -> usize {
    let dir = (*fd).dir as *mut Ext2OpenFd;
    COMBINE_64((*dir).inode.size_high, (*dir).inode.size)
}

pub unsafe fn ext2StatInternal(
    ext2: *mut Ext2,
    inode: *mut Ext2Inode,
    inodeNum: u32,
    target: *mut stat,
) {
    (*target).st_dev = 69;
    (*target).st_ino = inodeNum as u64;
    (*target).st_mode = (*inode).permission as u32;
    (*target).st_nlink = (*inode).hard_links as u32;
    (*target).st_uid = 0;
    (*target).st_gid = 0;
    (*target).st_rdev = 0;
    (*target).st_blksize = (*ext2).blockSize;
    (*target).st_size = COMBINE_64((*inode).size_high, (*inode).size);
    (*target).st_blocks =
        (DivRoundUp((*target).st_size, (*target).st_blksize) * (*target).st_blksize) / 512;
    (*target).st_atime = (*inode).atime;
    (*target).st_mtime = (*inode).mtime;
    (*target).st_ctime = (*inode).ctime;
}

//
// ===================== HANDLERS TABLE =====================
//

#[repr(C)]
pub struct VfsHandlers {
    pub open: unsafe extern "C" fn(),
    pub write: unsafe extern "C" fn(),
    pub close: unsafe extern "C" fn(),
    pub duplicate: unsafe extern "C" fn(),
    pub read: unsafe extern "C" fn(),
    pub stat: unsafe extern "C" fn(),
    pub getdents64: unsafe extern "C" fn(),
    pub seek: unsafe extern "C" fn(),
    pub getFilesize: unsafe extern "C" fn(),
    pub mmap: unsafe extern "C" fn(),
}

#[no_mangle]
pub static ext2Handlers: VfsHandlers = VfsHandlers {
    open: core::mem::transmute(0usize),
    write: core::mem::transmute(0usize),
    close: core::mem::transmute(0usize),
    duplicate: core::mem::transmute(0usize),
    read: core::mem::transmute(0usize),
    stat: core::mem::transmute(0usize),
    getdents64: core::mem::transmute(0usize),
    seek: core::mem::transmute(0usize),
    getFilesize: core::mem::transmute(0usize),
    mmap: core::mem::transmute(0usize),
};
