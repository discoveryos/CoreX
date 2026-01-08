#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;

//
// Constants (must match your C headers)
//

const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;

const S_IRUSR: u32 = 0o400;
const S_IWUSR: u32 = 0o200;
const S_IXUSR: u32 = 0o100;

const FAT_ATTRIB_DIRECTORY: u8 = 0x10;

//
// Structs (partial, ABI-compatible)
//

#[repr(C)]
pub struct FAT32 {
    pub bootsec: FAT32BootSector,
}

#[repr(C)]
pub struct FAT32BootSector {
    pub extended_section: FAT32Extended,
}

#[repr(C)]
pub struct FAT32Extended {
    pub root_cluster: u32,
}

#[repr(C)]
pub struct FAT32DirectoryEntry {
    pub attrib: u8,
    pub _reserved: [u8; 7],
    pub createdate: u16,
    pub createtime: u16,
    pub accessdate: u16,
    pub modifiedtime: u16,
    pub modifieddate: u16,
    pub clusterhigh: u16,
    pub clusterlow: u16,
    pub filesize: u32,
}

#[repr(C)]
pub struct FAT32TraverseResult {
    pub directory: u32,
    pub index: u32,
    pub dirEntry: FAT32DirectoryEntry,
}

#[repr(C)]
pub struct FAT32OpenFd {
    pub directoryStarting: u32,
    pub index: u32,
    pub dirEnt: FAT32DirectoryEntry,
}

#[repr(C)]
pub struct MountPoint {
    pub fsInfo: *mut c_void,
}

#[repr(C)]
pub struct OpenFile {
    pub dir: *mut c_void,
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
    pub st_size: u64,
    pub st_blksize: u64,
    pub st_blocks: u64,
    pub st_atime: i64,
    pub st_mtime: i64,
    pub st_ctime: i64,
}

//
// Externals
//

extern "C" {
    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);

    fn fat32TraversePath(
        fat: *mut FAT32,
        path: *const u8,
        root_cluster: u32,
    ) -> FAT32TraverseResult;

    fn fat32UnixTime(date: u16, time: u16) -> i64;

    fn FAT_INODE_GEN(directory: u32, index: u32) -> u64;
}

//
// Internal stat helper
//

#[no_mangle]
pub unsafe extern "C" fn fat32StatInternal(
    res: *const FAT32TraverseResult,
    target: *mut stat,
) {
    (*target).st_dev = 69; // haha
    (*target).st_ino = FAT_INODE_GEN((*res).directory, (*res).index);

    (*target).st_mode =
        S_IFREG | S_IRUSR | S_IWUSR | S_IXUSR;

    (*target).st_nlink = 1;
    (*target).st_uid = 0;
    (*target).st_gid = 0;
    (*target).st_rdev = 0;
    (*target).st_blksize = 0x1000;

    if ((*res).dirEntry.attrib & FAT_ATTRIB_DIRECTORY) != 0 {
        (*target).st_size = 0x1000;

        (*target).st_mode &= !S_IFREG;
        (*target).st_mode |= S_IFDIR;
    } else {
        (*target).st_size = (*res).dirEntry.filesize as u64;
    }

    (*target).st_blocks =
        ((((*target).st_size + (*target).st_blksize - 1)
            / (*target).st_blksize)
            * (*target).st_blksize)
            / 512;

    (*target).st_atime =
        fat32UnixTime((*res).dirEntry.accessdate, 0);

    (*target).st_mtime =
        fat32UnixTime(
            (*res).dirEntry.modifieddate,
            (*res).dirEntry.modifiedtime,
        );

    (*target).st_ctime =
        fat32UnixTime(
            (*res).dirEntry.createdate,
            (*res).dirEntry.createtime,
        );
}

//
// fat32Stat(path-based)
//

#[no_mangle]
pub unsafe extern "C" fn fat32Stat(
    mnt: *mut MountPoint,
    filename: *const u8,
    target: *mut stat,
    _symlinkResolve: *mut *mut u8,
) -> bool {
    let fat = (*mnt).fsInfo as *mut FAT32;

    let res = fat32TraversePath(
        fat,
        filename,
        (*fat).bootsec.extended_section.root_cluster,
    );

    if res.directory == 0 {
        return false;
    }

    fat32StatInternal(&res, target);
    true
}

//
// fat32StatFd (fd-based)
//

#[no_mangle]
pub unsafe extern "C" fn fat32StatFd(
    fd: *mut OpenFile,
    target: *mut stat,
) -> usize {
    let dir = (*fd).dir as *mut FAT32OpenFd;

    let mut res = FAT32TraverseResult {
        directory: (*dir).directoryStarting,
        index: (*dir).index,
        dirEntry: core::mem::zeroed(),
    };

    memcpy(
        &mut res.dirEntry as *mut _ as *mut c_void,
        &(*dir).dirEnt as *const _ as *const c_void,
        core::mem::size_of::<FAT32DirectoryEntry>(),
    );

    fat32StatInternal(&res, target);
    0
}
