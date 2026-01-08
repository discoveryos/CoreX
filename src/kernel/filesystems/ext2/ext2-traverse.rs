#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

extern crate alloc;

use alloc::alloc::{alloc, dealloc};
use core::ffi::c_void;
use core::mem::{zeroed};
use core::ptr::{copy_nonoverlapping};

//
// Constants
//

const SECTOR_SIZE: usize = 512;

const EXT2_S_IFLNK: u32 = 0xA000;
const S_IFDIR: u32 = 0x4000;

//
// Structs
//

#[repr(C)]
pub struct Ext2 {
    pub blockSize: usize,
}

#[repr(C)]
pub struct Ext2Inode {
    pub permission: u32,
    pub size: u32,
    pub blocks: [u32; 15],
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

//
// External helpers
//

extern "C" {
    fn ext2InodeFetch(ext2: *mut Ext2, inode: usize) -> *mut Ext2Inode;

    fn ext2BlockFetchInit(ext2: *mut Ext2, ctrl: *mut Ext2LookupControl);
    fn ext2BlockFetchCleanup(ctrl: *mut Ext2LookupControl);

    fn ext2BlockFetch(
        ext2: *mut Ext2,
        inode: *mut Ext2Inode,
        inode_num: usize,
        ctrl: *mut Ext2LookupControl,
        block_num: usize,
    ) -> usize;

    fn getDiskBytes(buf: *mut u8, lba: usize, sectors: usize);

    fn free(ptr: *mut c_void);
}

//
// Helpers
//

#[inline]
fn DivRoundUp(a: u32, b: usize) -> usize {
    ((a as usize) + b - 1) / b
}

#[inline]
fn BLOCK_TO_LBA(_ext2: *mut Ext2, _bg: u32, block: usize) -> usize {
    block
}

#[inline]
fn strlength(s: *const u8) -> usize {
    let mut len = 0usize;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

//
// ext2Traverse
//

#[no_mangle]
pub unsafe extern "C" fn ext2Traverse(
    ext2: *mut Ext2,
    initInode: usize,
    search: *const u8,
    searchLength: usize,
) -> u32 {
    let mut ret: u32 = 0;

    let ino = ext2InodeFetch(ext2, initInode);
    let layout =
        core::alloc::Layout::from_size_align((*ext2).blockSize, 1).unwrap();
    let names = alloc(layout);

    let mut control: Ext2LookupControl = zeroed();
    let mut block_num = 0usize;

    ext2BlockFetchInit(ext2, &mut control);

    let blocks = DivRoundUp((*ino).size, (*ext2).blockSize);
    for _ in 0..blocks {
        let block = ext2BlockFetch(ext2, ino, initInode, &mut control, block_num);
        block_num += 1;

        if block == 0 {
            break;
        }

        getDiskBytes(
            names,
            BLOCK_TO_LBA(ext2, 0, block),
            (*ext2).blockSize / SECTOR_SIZE,
        );

        let mut dir = names as *mut Ext2Directory;

        while (dir as usize - names as usize) < (*ext2).blockSize {
            if (*dir).inode == 0 {
                dir = (dir as usize + (*dir).size as usize) as *mut _;
                continue;
            }

            if (*dir).filenameLength as usize == searchLength {
                let fname =
                    core::slice::from_raw_parts((*dir).filename.as_ptr(), searchLength);
                let sname = core::slice::from_raw_parts(search, searchLength);

                if fname == sname {
                    ret = (*dir).inode;
                    goto_cleanup!(());
                }
            }

            dir = (dir as usize + (*dir).size as usize) as *mut _;
        }
    }

cleanup:
    ext2BlockFetchCleanup(&mut control);
    free(ino as *mut c_void);
    dealloc(names, layout);
    ret
}

//
// ext2TraversePath
//

#[no_mangle]
pub unsafe extern "C" fn ext2TraversePath(
    ext2: *mut Ext2,
    path: *const u8,
    initInode: usize,
    follow: bool,
    symlinkResolve: *mut *mut u8,
) -> u32 {
    let mut curr = initInode;
    let len = strlength(path);

    // opening "/"
    if len == 1 {
        return 2;
    }

    let mut lastslash = 0usize;

    for i in 1..len {
        let last = i == len - 1;

        if *path.add(i) == b'/' || last {
            let mut length = i - lastslash - 1;
            if last {
                length += 1;
            }

            curr = ext2Traverse(ext2, curr, path.add(lastslash + 1), length);
            if curr == 0 {
                return 0;
            }

            let inode = ext2InodeFetch(ext2, curr);

            if ((*inode).permission & 0xF000) == EXT2_S_IFLNK && (!last || follow) {
                let mut start: *mut u8;
                let symlink_len = (*inode).size as usize;

                let target_len = len + symlink_len + 2;
                let symlink_target =
                    alloc(core::alloc::Layout::from_size_align(target_len, 1).unwrap());

                if (*inode).size > 60 {
                    start = alloc(
                        core::alloc::Layout::from_size_align(
                            (*ext2).blockSize + 1,
                            1,
                        )
                        .unwrap(),
                    );

                    getDiskBytes(
                        start,
                        BLOCK_TO_LBA(ext2, 0, (*inode).blocks[0] as usize),
                        (*ext2).blockSize / SECTOR_SIZE,
                    );
                } else {
                    start = (*inode).blocks.as_mut_ptr() as *mut u8;
                }

                *symlinkResolve = symlink_target;

                if *start != b'/' {
                    copy_nonoverlapping(path, symlink_target, lastslash + 1);
                    copy_nonoverlapping(
                        start,
                        symlink_target.add(lastslash + 1),
                        symlink_len,
                    );
                    copy_nonoverlapping(
                        path.add(lastslash + 1 + length),
                        symlink_target.add(lastslash + 1 + symlink_len),
                        len - (lastslash + 1 + length),
                    );
                } else {
                    *symlink_target = b'!';
                    copy_nonoverlapping(start, symlink_target.add(1), symlink_len);
                }

                if (*inode).size > 60 {
                    dealloc(
                        start,
                        core::alloc::Layout::from_size_align(
                            (*ext2).blockSize + 1,
                            1,
                        )
                        .unwrap(),
                    );
                }

                free(inode as *mut c_void);
                return 0;
            }

            let notdir = ((*inode).permission & S_IFDIR) == 0;
            free(inode as *mut c_void);

            if i == len - 1 {
                return curr;
            }

            if notdir {
                return 0;
            }

            lastslash = i;
        }
    }

    0
}
