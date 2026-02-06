#![no_std]

use core::cmp::{min};
use core::mem::size_of;
use core::ptr::{null_mut};
use core::slice;

//
// Constants & helpers
//

pub const S_IFDIR: u16 = 0o040000;
pub const S_IFLNK: u16 = 0o120000;

pub const S_IRUSR: u16 = 0o400;
pub const S_IWUSR: u16 = 0o200;
pub const S_IXUSR: u16 = 0o100;

fn div_round_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

//
// Linked list primitives (minimal, intrusive)
//

#[repr(C)]
pub struct LlNode {
    pub next: *mut LlNode,
}

#[repr(C)]
pub struct LlControl {
    pub first_object: *mut LlNode,
}

pub unsafe fn ll_init(ctrl: *mut LlControl) {
    (*ctrl).first_object = null_mut();
}

pub unsafe fn ll_allocate<T>(ctrl: *mut LlControl) -> *mut T {
    let obj = crate::malloc::malloc(size_of::<T>()) as *mut T;
    let node = obj as *mut LlNode;

    (*node).next = (*ctrl).first_object;
    (*ctrl).first_object = node;

    obj
}

//
// VFS structures
//

pub struct MountPoint {
    pub fs_info: *mut FakefsOverlay,
}

pub struct OpenFile {
    pub mount_point: *mut MountPoint,
    pub handlers: *const VfsHandlers,
    pub dirname: *mut u8,
    pub pointer: usize,
    pub fakefs: *mut FakefsFile,
    pub tmp1: usize,
}

#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u16,
    pub st_nlink: u64,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: usize,
    pub st_blksize: usize,
    pub st_blocks: usize,
    pub st_atime: u64,
    pub st_mtime: u64,
    pub st_ctime: u64,
}

//
// FakeFS structures
//

#[repr(C)]
pub struct Fakefs {
    pub root_file: LlControl,
    pub last_inode: u64,
}

#[repr(C)]
pub struct FakefsOverlay {
    pub fakefs: *mut Fakefs,
}

#[repr(C)]
pub struct FakefsFile {
    pub inner: LlControl,
    pub ll: LlNode,

    pub filename: *const u8,
    pub filename_length: usize,

    pub symlink: *const u8,
    pub symlink_length: usize,

    pub filetype: u16,
    pub inode: u64,
    pub size: usize,

    pub handlers: *const VfsHandlers,
    pub extra: *mut u8,
}

//
// VFS handlers
//

pub struct VfsHandlers {
    pub open: Option<unsafe fn(*const u8, i32, i32, *mut OpenFile, *mut *mut u8) -> isize>,
    pub read: Option<unsafe fn(*mut OpenFile, *mut u8, usize) -> usize>,
    pub seek: Option<unsafe fn(*mut OpenFile, usize, isize, i32) -> usize>,
    pub stat: Option<unsafe fn(*mut OpenFile, *mut Stat) -> usize>,
    pub getdents64: Option<unsafe fn(*mut OpenFile, *mut u8, u32) -> usize>,
}

//
// Core FakeFS logic
//

pub unsafe fn fakefs_add_file(
    fakefs: *mut Fakefs,
    under: *mut FakefsFile,
    filename: *const u8,
    filename_len: usize,
    symlink: *const u8,
    symlink_len: usize,
    filetype: u16,
    handlers: *const VfsHandlers,
) -> *mut FakefsFile {
    let file = ll_allocate::<FakefsFile>(&mut (*under).inner);

    ll_init(&mut (*file).inner);

    (*file).filename = filename;
    (*file).filename_length = filename_len;
    (*file).filetype = filetype;
    (*fakefs).last_inode += 1;
    (*file).inode = (*fakefs).last_inode;
    (*file).handlers = handlers;

    if !symlink.is_null() {
        (*file).symlink = symlink;
        (*file).symlink_length = symlink_len;
    } else {
        (*file).symlink = null_mut();
        (*file).symlink_length = 0;
    }

    file
}

pub unsafe fn fakefs_attach_file(file: *mut FakefsFile, ptr: *mut u8, size: usize) {
    (*file).extra = ptr;
    (*file).size = size;
}

unsafe fn filename_eq(a: *const u8, alen: usize, b: *const u8, blen: usize) -> bool {
    if alen != blen {
        return false;
    }

    let sa = slice::from_raw_parts(a, alen);
    let sb = slice::from_raw_parts(b, blen);
    sa == sb
}

pub unsafe fn fakefs_traverse(
    start: *mut FakefsFile,
    search: *const u8,
    search_len: usize,
) -> *mut FakefsFile {
    let mut browse = start;

    while !browse.is_null() {
        if *(*browse).filename == b'*' {
            break;
        }

        if filename_eq(
            search,
            search_len,
            (*browse).filename,
            (*browse).filename_length,
        ) {
            break;
        }

        browse = (*browse).ll.next as *mut FakefsFile;
    }

    browse
}

pub unsafe fn fakefs_setup_root(root: *mut LlControl) {
    ll_init(root);

    let root_file = ll_allocate::<FakefsFile>(root);

    (*root_file).filename = b"/\0".as_ptr();
    (*root_file).filename_length = 1;
    (*root_file).filetype = S_IFDIR | S_IRUSR | S_IWUSR | S_IXUSR;
    (*root_file).symlink = null_mut();
    (*root_file).symlink_length = 0;
    (*root_file).handlers = &FAKEFS_ROOT_HANDLERS;
    (*root_file).size = 3620;
    (*root_file).inode = 1;

    ll_init(&mut (*root_file).inner);
}

pub unsafe fn fakefs_traverse_path(
    start: *mut FakefsFile,
    path: *const u8,
    path_len: usize,
) -> *mut FakefsFile {
    if path_len == 1 {
        return start;
    }

    let mut current = (*start).inner.first_object as *mut FakefsFile;
    let mut last_slash = 0usize;

    for i in 1..path_len {
        let last = i == path_len - 1;

        if *path.add(i) == b'/' || last {
            let mut len = i - last_slash - 1;
            if last {
                len += 1;
            }

            let res = fakefs_traverse(
                current,
                path.add(last_slash + 1),
                len,
            );

            if res.is_null() || last {
                return res;
            }

            current = (*res).inner.first_object as *mut FakefsFile;
            last_slash = i;
        }
    }

    null_mut()
}

//
// Stat
//

pub unsafe fn fakefs_stat_generic(file: *mut FakefsFile, st: *mut Stat) {
    (*st).st_dev = 69;
    (*st).st_ino = (*file).inode;
    (*st).st_mode = (*file).filetype;
    (*st).st_nlink = 1;
    (*st).st_uid = 0;
    (*st).st_gid = 0;
    (*st).st_rdev = 0;
    (*st).st_blksize = 0x1000;
    (*st).st_size = (*file).size;
    (*st).st_blocks =
        div_round_up((*st).st_size, (*st).st_blksize) * (*st).st_blksize / 512;
    (*st).st_atime = 0;
    (*st).st_mtime = 0;
    (*st).st_ctime = 0;
}

//
// Simple read helpers
//

pub unsafe fn fakefs_simple_read(
    fd: *mut OpenFile,
    out: *mut u8,
    limit: usize,
) -> usize {
    let file = (*fd).fakefs;
    let data = (*file).extra.add((*fd).pointer);

    let mut count = 0usize;

    for i in 0..limit {
        let c = *data.add(i);
        if c == 0 {
            break;
        }

        *out.add(i) = c;
        (*fd).pointer += 1;
        count += 1;
    }

    count
}

pub unsafe fn fakefs_simple_seek(
    fd: *mut OpenFile,
    target: usize,
    _offset: isize,
    _whence: i32,
) -> usize {
    (*fd).pointer = target;
    0
}

//
// Handlers
//

pub static FAKEFS_NO_HANDLERS: VfsHandlers = VfsHandlers {
    open: None,
    read: None,
    seek: None,
    stat: None,
    getdents64: None,
};

pub static FAKEFS_ROOT_HANDLERS: VfsHandlers = VfsHandlers {
    open: None,
    read: None,
    seek: None,
    stat: None,
    getdents64: None,
};

pub static FAKEFS_SIMPLE_READ_HANDLERS: VfsHandlers = VfsHandlers {
    open: None,
    read: Some(fakefs_simple_read),
    seek: Some(fakefs_simple_seek),
    stat: None,
    getdents64: None,
};
