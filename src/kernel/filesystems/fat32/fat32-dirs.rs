#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;

//
// Constants
//

const FAT_ATTRIB_DIRECTORY: u8 = 0x10;
const FAT_ATTRIB_ARCHIVE: u8 = 0x20;
const FAT_ATTRIB_LFN: u8 = 0x0F;

const LFN_ORDER_FINAL: u8 = 0x40;

const CDT_DIR: u8 = 4;
const CDT_REG: u8 = 8;

//
// Structs
//

#[repr(C)]
pub struct OpenFile {
    pub mountPoint: *mut MountPoint,
    pub dir: *mut c_void,
    pub dirname: *mut u8,
}

#[repr(C)]
pub struct MountPoint {
    pub fsInfo: *mut c_void,
}

#[repr(C)]
pub struct FAT32 {
    pub bootsec: FAT32BootSector,
}

#[repr(C)]
pub struct FAT32BootSector {
    pub sectors_per_cluster: u8,
}

#[repr(C)]
pub struct FAT32OpenFd {
    pub ptr: usize,
    pub directoryCurr: u32,
    pub dirEnt: FAT32DirectoryEntry,
}

#[repr(C)]
pub struct FAT32DirectoryEntry {
    pub name: [u8; 11],
    pub attrib: u8,
}

#[repr(C)]
pub struct FAT32LFN {
    pub order: u8,
    pub name1: [u16; 5],
    pub attrib: u8,
    pub r#type: u8,
    pub checksum: u8,
    pub name2: [u16; 6],
    pub zero: u16,
    pub name3: [u16; 2],
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
// Helper macros
//

#[inline]
fn FAT_PTR(ptr: *mut c_void) -> *mut FAT32 {
    ptr as *mut FAT32
}

#[inline]
fn FAT_DIR_PTR(ptr: *mut c_void) -> *mut FAT32OpenFd {
    ptr as *mut FAT32OpenFd
}

#[inline]
fn LBA_TO_OFFSET(sectors: u8) -> usize {
    sectors as usize * 512
}

//
// External symbols
//

extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut c_void);
    fn memset(ptr: *mut c_void, val: i32, size: usize);

    fn getDiskBytes(buf: *mut u8, lba: u32, sectors: u32);

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn fat32ClusterToLBA(fat: *mut FAT32, cluster: u32) -> u32;
    fn fat32FATtraverse(fat: *mut FAT32, curr: u32) -> u32;

    fn fat32LFNmemcpy(dst: *mut u8, lfn: *const FAT32LFN, index: i32);
    fn fat32SFNtoNormal(dst: *mut u8, dir: *const FAT32DirectoryEntry) -> i32;

    fn dentsAdd(
        start: *mut linux_dirent64,
        dirp: *mut *mut linux_dirent64,
        allocated: *mut usize,
        hardlimit: u32,
        name: *mut c_void,
        namelen: i32,
        inode: u64,
        dtype: u8,
    ) -> DENTS_RES;
}

//
// Enums
//

#[repr(C)]
#[derive(PartialEq, Eq)]
pub enum DENTS_RES {
    DENTS_OK = 0,
    DENTS_NO_SPACE = 1,
    DENTS_RETURN = 2,
}

//
// Macros/constants expected from headers
//

const LFN_MAX: i32 = 20;
const LFN_MAX_TOTAL_CHARS: usize = 260;

#[inline]
fn FAT_INODE_GEN(cluster: u32, index: u32) -> u64 {
    ((cluster as u64) << 32) | index as u64
}

#[inline]
fn ERR(code: usize) -> usize {
    (code as isize as usize)
}

const ENOTDIR: usize = 20;
const EINVAL: usize = 22;

//
// fat32Getdents64
//

#[no_mangle]
pub unsafe extern "C" fn fat32Getdents64(
    file: *mut OpenFile,
    start: *mut linux_dirent64,
    hardlimit: u32,
) -> usize {
    let fat = FAT_PTR((*(*file).mountPoint).fsInfo);
    let fat_dir = FAT_DIR_PTR((*file).dir);

    if (*fat_dir).dirEnt.attrib & FAT_ATTRIB_DIRECTORY == 0 {
        return ERR(ENOTDIR);
    }

    if (*fat_dir).directoryCurr == 0 {
        return 0;
    }

    let mut allocatedlimit: usize = 0;
    let bytes_per_cluster = LBA_TO_OFFSET((*fat).bootsec.sectors_per_cluster);

    let mut lfn_name = [0u8; LFN_MAX_TOTAL_CHARS];
    let mut lfn_last: i32 = -1;

    let bytes = malloc(bytes_per_cluster);
    let mut dirp = start;

    'outer: loop {
        if (*fat_dir).directoryCurr == 0 {
            break;
        }

        let offset_start = (*fat_dir).ptr % bytes_per_cluster;

        getDiskBytes(
            bytes,
            fat32ClusterToLBA(fat, (*fat_dir).directoryCurr),
            (*fat).bootsec.sectors_per_cluster as u32,
        );

        let mut i = offset_start;
        while i < bytes_per_cluster {
            let dir =
                &*(bytes.add(i) as *const FAT32DirectoryEntry);
            let lfn =
                &*(dir as *const _ as *const FAT32LFN);

            if dir.attrib == FAT_ATTRIB_LFN && lfn.r#type == 0 {
                let index = (lfn.order & !LFN_ORDER_FINAL) as i32 - 1;

                if index >= LFN_MAX {
                    debugf(
                        b"[fat32] Invalid LFN index{%d} size!\n\0".as_ptr(),
                        index,
                    );
                    panic();
                }

                if (lfn.order & LFN_ORDER_FINAL) != 0 {
                    lfn_last = index;
                }

                fat32LFNmemcpy(lfn_name.as_mut_ptr(), lfn, index);
            }

            if dir.attrib == FAT_ATTRIB_DIRECTORY
                || dir.attrib == FAT_ATTRIB_ARCHIVE
            {
                let lfn_len: i32;

                if lfn_last >= 0 {
                    let mut len = 0;
                    while lfn_name[len] != 0 {
                        len += 1;
                    }
                    lfn_len = len as i32;
                    lfn_last = -1;
                } else {
                    lfn_len =
                        fat32SFNtoNormal(lfn_name.as_mut_ptr(), dir);
                }

                let dtype = if dir.attrib & FAT_ATTRIB_DIRECTORY != 0 {
                    CDT_DIR
                } else {
                    CDT_REG
                };

                let res = dentsAdd(
                    start,
                    &mut dirp,
                    &mut allocatedlimit,
                    hardlimit,
                    lfn_name.as_mut_ptr() as *mut c_void,
                    lfn_len,
                    FAT_INODE_GEN(
                        (*fat_dir).directoryCurr,
                        (i / 32) as u32,
                    ),
                    dtype,
                );

                if res == DENTS_RES::DENTS_NO_SPACE {
                    allocatedlimit = ERR(EINVAL);
                    break 'outer;
                } else if res == DENTS_RES::DENTS_RETURN {
                    break 'outer;
                }

                memset(
                    lfn_name.as_mut_ptr() as *mut c_void,
                    0,
                    LFN_MAX_TOTAL_CHARS,
                );
            }

            (*fat_dir).ptr += core::mem::size_of::<FAT32DirectoryEntry>();
            i += core::mem::size_of::<FAT32DirectoryEntry>();
        }

        (*fat_dir).directoryCurr =
            fat32FATtraverse(fat, (*fat_dir).directoryCurr);

        if (*fat_dir).directoryCurr == 0 {
            break;
        }
    }

    free(bytes as *mut c_void);
    allocatedlimit
}
