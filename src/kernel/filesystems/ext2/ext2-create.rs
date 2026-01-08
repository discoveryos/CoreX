#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

extern crate alloc;

use alloc::alloc::{alloc, dealloc};
use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::{copy_nonoverlapping, null_mut};

//
// Constants
//

const EXT2_ROOT_INODE: u32 = 2;
const SECTOR_SIZE: u32 = 512;

const S_IFMT: u32  = 0o170000;
const S_IFDIR: u32 = 0o040000;
const S_IFREG: u32 = 0o100000;

const ENOENT: usize = 2;
const EEXIST: usize = 17;
const ENOTDIR: usize = 20;
const EISDIR: usize = 21;

#[inline]
fn ERR(e: usize) -> usize {
    (-(e as isize)) as usize
}

//
// Extern globals (from kernel)
//

extern "C" {
    static timerBootUnix: usize;
    static timerTicks: usize;
}

//
// Structs
//

#[repr(C)]
pub struct MountPoint {
    pub fsInfo: *mut c_void,
}

#[repr(C)]
pub struct Ext2 {
    _unused: u8,
}

#[repr(C)]
pub struct Ext2Inode {
    pub permission: u32,
    pub userid: u16,
    pub size: u32,
    pub atime: usize,
    pub ctime: usize,
    pub mtime: usize,
    pub dtime: usize,
    pub gid: u16,
    pub hard_links: u16,
    pub num_sectors: u32,
    pub generation: u32,
    pub file_acl: u32,
    pub dir_acl: u32,
    pub f_block_addr: u32,
    pub size_high: u32,
}

//
// External ext2 helpers
//

extern "C" {
    fn ext2TraversePath(
        fs: *mut Ext2,
        path: *const u8,
        start_inode: u32,
        follow: bool,
        symlink_resolve: *mut *mut u8,
    ) -> u32;

    fn ext2Traverse(
        fs: *mut Ext2,
        inode: u32,
        name: *const u8,
        name_len: i32,
    ) -> u32;

    fn ext2InodeFetch(fs: *mut Ext2, inode: u32) -> *mut Ext2Inode;
    fn ext2InodeModifyM(fs: *mut Ext2, inode: u32, data: *const Ext2Inode);

    fn ext2InodeFind(fs: *mut Ext2, group: u32) -> u32;

    fn ext2DirAllocate(
        fs: *mut Ext2,
        parent_inode: u32,
        parent_data: *mut Ext2Inode,
        name: *const u8,
        name_len: i32,
        file_type: u8,
        inode: u32,
    );
}

#[inline]
fn EXT2_PTR(ptr: *mut c_void) -> *mut Ext2 {
    ptr as *mut Ext2
}

#[inline]
fn INODE_TO_BLOCK_GROUP(_fs: *mut Ext2, inode: u32) -> u32 {
    inode // placeholder, matches original semantics
}

//
// Utility
//

fn strlength(s: *const u8) -> i32 {
    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

//
// ext2Mkdir
//

#[no_mangle]
pub unsafe extern "C" fn ext2Mkdir(
    mnt: *mut MountPoint,
    dirname: *mut u8,
    mode: u32,
    symlinkResolve: *mut *mut u8,
) -> usize {
    let ext2 = EXT2_PTR((*mnt).fsInfo);

    let len = strlength(dirname);
    let mut last_slash = -1;

    for i in 0..len {
        if *dirname.add(i as usize) == b'/' {
            last_slash = i;
        }
    }

    let inode: u32;

    if last_slash > 0 {
        let buf_size = (last_slash + 1) as usize;
        let parent = alloc(core::alloc::Layout::from_size_align(buf_size, 1).unwrap()) as *mut u8;
        copy_nonoverlapping(dirname, parent, last_slash as usize);
        *parent.add(last_slash as usize) = 0;

        inode = ext2TraversePath(
            ext2,
            parent,
            EXT2_ROOT_INODE,
            true,
            symlinkResolve,
        );

        dealloc(parent, core::alloc::Layout::from_size_align(buf_size, 1).unwrap());
    } else {
        inode = EXT2_ROOT_INODE;
    }

    if inode == 0 {
        return ERR(ENOENT);
    }

    let name = if last_slash >= 0 {
        dirname.add((last_slash + 1) as usize)
    } else {
        dirname
    };

    let name_len = strlength(name);
    if name_len == 0 {
        return ERR(EEXIST);
    }

    let inode_contents = ext2InodeFetch(ext2, inode);
    if ((*inode_contents).permission & S_IFMT) != S_IFDIR {
        dealloc(inode_contents as *mut u8,
            core::alloc::Layout::from_size_align(size_of::<Ext2Inode>(), 1).unwrap());
        return ERR(ENOTDIR);
    }

    if ext2Traverse(ext2, inode, name, name_len) != 0 {
        dealloc(inode_contents as *mut u8,
            core::alloc::Layout::from_size_align(size_of::<Ext2Inode>(), 1).unwrap());
        return ERR(EEXIST);
    }

    let time = timerBootUnix + timerTicks / 1000;

    let mut new_inode: Ext2Inode = core::mem::zeroed();
    new_inode.permission = S_IFDIR | mode;
    new_inode.atime = time;
    new_inode.ctime = time;
    new_inode.mtime = time;
    new_inode.hard_links = 2;
    new_inode.num_sectors = 0;

    let group = INODE_TO_BLOCK_GROUP(ext2, inode);
    let new_inode_num = ext2InodeFind(ext2, group);

    ext2InodeModifyM(ext2, new_inode_num, &new_inode);

    ext2DirAllocate(ext2, new_inode_num, &mut new_inode, b".\0".as_ptr(), 1, 2, new_inode_num);
    ext2DirAllocate(ext2, new_inode_num, &mut new_inode, b"..\0".as_ptr(), 2, 2, inode);

    ext2DirAllocate(ext2, inode, inode_contents, name, name_len, 2, new_inode_num);
    (*inode_contents).hard_links += 1;

    ext2InodeModifyM(ext2, inode, inode_contents);

    dealloc(inode_contents as *mut u8,
        core::alloc::Layout::from_size_align(size_of::<Ext2Inode>(), 1).unwrap());

    0
}

//
// ext2Touch
//

#[no_mangle]
pub unsafe extern "C" fn ext2Touch(
    mnt: *mut MountPoint,
    filename: *mut u8,
    mode: u32,
    symlinkResolve: *mut *mut u8,
) -> usize {
    let ext2 = EXT2_PTR((*mnt).fsInfo);

    let len = strlength(filename);
    let mut last_slash = -1;

    for i in 0..len {
        if *filename.add(i as usize) == b'/' {
            last_slash = i;
        }
    }

    let inode: u32;

    if last_slash > 0 {
        let buf_size = (last_slash + 1) as usize;
        let parent = alloc(core::alloc::Layout::from_size_align(buf_size, 1).unwrap()) as *mut u8;
        copy_nonoverlapping(filename, parent, last_slash as usize);
        *parent.add(last_slash as usize) = 0;

        inode = ext2TraversePath(
            ext2,
            parent,
            EXT2_ROOT_INODE,
            true,
            symlinkResolve,
        );

        dealloc(parent, core::alloc::Layout::from_size_align(buf_size, 1).unwrap());
    } else {
        inode = EXT2_ROOT_INODE;
    }

    if inode == 0 {
        return ERR(ENOENT);
    }

    let name = if last_slash >= 0 {
        filename.add((last_slash + 1) as usize)
    } else {
        filename
    };

    let name_len = strlength(name);
    if name_len == 0 {
        return ERR(EISDIR);
    }

    let inode_contents = ext2InodeFetch(ext2, inode);
    if ((*inode_contents).permission & S_IFMT) != S_IFDIR {
        dealloc(inode_contents as *mut u8,
            core::alloc::Layout::from_size_align(size_of::<Ext2Inode>(), 1).unwrap());
        return ERR(ENOTDIR);
    }

    if ext2Traverse(ext2, inode, name, name_len) != 0 {
        dealloc(inode_contents as *mut u8,
            core::alloc::Layout::from_size_align(size_of::<Ext2Inode>(), 1).unwrap());
        return ERR(EEXIST);
    }

    let time = timerBootUnix + timerTicks / 1000;

    let mut new_inode: Ext2Inode = core::mem::zeroed();
    new_inode.permission = S_IFREG | mode;
    new_inode.atime = time;
    new_inode.ctime = time;
    new_inode.mtime = time;
    new_inode.hard_links = 1;

    let group = INODE_TO_BLOCK_GROUP(ext2, inode);
    let new_inode_num = ext2InodeFind(ext2, group);

    ext2InodeModifyM(ext2, new_inode_num, &new_inode);

    ext2DirAllocate(ext2, inode, inode_contents, name, name_len, 1, new_inode_num);

    dealloc(inode_contents as *mut u8,
        core::alloc::Layout::from_size_align(size_of::<Ext2Inode>(), 1).unwrap());

    0
}
