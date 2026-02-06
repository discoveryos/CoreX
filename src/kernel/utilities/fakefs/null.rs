// nullfs.rs
// /dev/null fake filesystem handler
// Converted from C to Rust

#![no_std]

use core::ptr;

/* =======================
   Types
   ======================= */

#[repr(C)]
pub struct OpenFile {
    _private: [u8; 0],
}

#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_blksize: u64,
    pub st_size: u64,
    pub st_blocks: u64,
    pub st_atime: u64,
    pub st_mtime: u64,
    pub st_ctime: u64,
}

#[repr(C)]
pub struct VfsHandlers {
    pub read: Option<extern "C" fn(*mut OpenFile, *mut u8, usize) -> usize>,
    pub write: Option<extern "C" fn(*mut OpenFile, *const u8, usize) -> usize>,
    pub stat: Option<extern "C" fn(*mut OpenFile, *mut Stat) -> usize>,
    pub duplicate: Option<extern "C" fn() -> bool>,
    pub ioctl: Option<extern "C" fn(*mut OpenFile, u64, *mut core::ffi::c_void) -> usize>,
    pub mmap: Option<extern "C" fn() -> usize>,
    pub getdents64: Option<extern "C" fn() -> usize>,
}

/* =======================
   Externs & constants
   ======================= */

extern "C" {
    fn rand() -> u64;
    fn debugf(fmt: *const u8, ...) -> i32;
}

const S_IFCHR: u32 = 0o020000;
const S_IRUSR: u32 = 0o400;
const S_IWUSR: u32 = 0o200;

const ENOTTY: usize = 25;

#[inline]
fn err(code: usize) -> usize {
    (!0usize).wrapping_sub(code - 1)
}

#[inline]
fn div_round_up(a: u64, b: u64) -> u64 {
    (a + b - 1) / b
}

/* =======================
   /dev/null handlers
   ======================= */

#[no_mangle]
pub extern "C" fn nullRead(
    _fd: *mut OpenFile,
    _out: *mut u8,
    _limit: usize,
) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn nullWrite(
    _fd: *mut OpenFile,
    _input: *const u8,
    limit: usize,
) -> usize {
    limit
}

#[no_mangle]
pub extern "C" fn nullStat(fd: *mut OpenFile, target: *mut Stat) -> usize {
    let _ = fd;

    unsafe {
        (*target).st_dev = 70;
        (*target).st_ino = rand(); // TODO: real inode
        (*target).st_mode = S_IFCHR | S_IRUSR | S_IWUSR;
        (*target).st_nlink = 1;
        (*target).st_uid = 0;
        (*target).st_gid = 0;
        (*target).st_rdev = 0;
        (*target).st_blksize = 0x1000;
        (*target).st_size = 0;
        (*target).st_blocks = div_round_up((*target).st_size, 512);
        (*target).st_atime = 69;
        (*target).st_mtime = 69;
        (*target).st_ctime = 69;
    }

    0
}

#[no_mangle]
pub extern "C" fn nullIoctl(
    _fd: *mut OpenFile,
    _request: u64,
    _arg: *mut core::ffi::c_void,
) -> usize {
    err(ENOTTY)
}

#[no_mangle]
pub extern "C" fn nullDuplicate() -> bool {
    true
}

#[no_mangle]
pub extern "C" fn nullMmap() -> usize {
    unsafe {
        debugf(b"[/dev/null] Tried to mmap?\n\0".as_ptr());
    }
    usize::MAX
}

/* =======================
   VFS registration
   ======================= */

#[no_mangle]
pub static handleNull: VfsHandlers = VfsHandlers {
    read: Some(nullRead),
    write: Some(nullWrite),
    stat: Some(nullStat),
    duplicate: Some(nullDuplicate),
    ioctl: Some(nullIoctl),
    mmap: Some(nullMmap),
    getdents64: None,
};
