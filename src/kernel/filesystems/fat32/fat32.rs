#![no_std]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

extern crate alloc;

use core::ptr::{copy_nonoverlapping, write_bytes};
use core::ffi::c_void;

//
// Constants
//

const SECTOR_SIZE: usize = 512;

const FAT_ATTRIB_DIRECTORY: u8 = 0x10;

const FAT32_CACHE_MAX: usize = 8;
const FAT32_CACHE_BAD: u32 = 0xFFFFFFFF;

//
// Structs
//

#[repr(C)]
pub struct MountPoint {
    pub handlers: *const VfsHandlers,
    pub stat: extern "C" fn(*mut OpenFile) -> usize,
    pub lstat: extern "C" fn(*mut OpenFile) -> usize,
    pub fsInfo: *mut c_void,
    pub mbr: MbrPartition,
}

#[repr(C)]
pub struct MbrPartition {
    pub lba_first_sector: u32,
}

#[repr(C)]
pub struct OpenFile {
    pub mountPoint: *mut MountPoint,
    pub dir: *mut c_void,
    pub dirname: *mut u8,
}

#[repr(C)]
pub struct FAT32 {
    pub offsetBase: u32,
    pub offsetFats: u32,
    pub offsetClusters: u32,

    pub bootsec: FAT32BootSector,

    pub cacheBase: [u32; FAT32_CACHE_MAX],
    pub cache: [*mut u8; FAT32_CACHE_MAX],
}

#[repr(C)]
pub struct FAT32BootSector {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sector_count: u16,
    pub table_count: u8,
    pub extended_section: FAT32BootExt,
}

#[repr(C)]
pub struct FAT32BootExt {
    pub table_size_32: u32,
    pub root_cluster: u32,
}

#[repr(C)]
pub struct FAT32DirectoryEntry {
    pub attrib: u8,
    pub clusterhigh: u16,
    pub clusterlow: u16,
    pub filesize: u32,
}

#[repr(C)]
pub struct FAT32OpenFd {
    pub ptr: usize,
    pub index: u32,
    pub directoryStarting: u32,
    pub directoryCurr: u32,
    pub dirEnt: FAT32DirectoryEntry,
}

#[repr(C)]
pub struct FAT32TraverseResult {
    pub directory: u32,
    pub index: u32,
    pub dirEntry: FAT32DirectoryEntry,
}

#[repr(C)]
pub struct VfsHandlers {
    pub open: extern "C" fn(*mut u8, i32, i32, *mut OpenFile, *mut *mut u8) -> usize,
    pub close: extern "C" fn(*mut OpenFile) -> bool,
    pub duplicate: extern "C" fn(*mut OpenFile, *mut OpenFile) -> bool,
    pub read: extern "C" fn(*mut OpenFile, *mut u8, usize) -> usize,
    pub stat: extern "C" fn(*mut OpenFile) -> usize,
    pub getdents64: extern "C" fn(*mut OpenFile, *mut c_void, u32) -> usize,
    pub seek: extern "C" fn(*mut OpenFile, usize, isize, i32) -> usize,
    pub getFilesize: extern "C" fn(*mut OpenFile) -> usize,
}

//
// Macros / helpers
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
fn DivRoundUp(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

#[inline]
fn LBA_TO_OFFSET(sectors: u8) -> usize {
    sectors as usize * SECTOR_SIZE
}

#[inline]
fn FAT_COMB_HIGH_LOW(h: u16, l: u16) -> u32 {
    ((h as u32) << 16) | (l as u32)
}

//
// Externs
//

extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut c_void);

    fn memset(ptr: *mut c_void, val: i32, size: usize);
    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);

    fn getDiskBytes(buf: *mut u8, lba: u32, sectors: u32);

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn strlength(s: *const u8) -> usize;

    fn fat32TraversePath(
        fat: *mut FAT32,
        path: *mut u8,
        start_cluster: u32,
    ) -> FAT32TraverseResult;

    fn fat32FATchain(fat: *mut FAT32, start: u32, count: i32) -> *mut u32;
    fn fat32FATtraverse(fat: *mut FAT32, curr: u32) -> u32;
    fn fat32ClusterToLBA(fat: *mut FAT32, cluster: u32) -> u32;

    fn fat32Stat(fd: *mut OpenFile) -> usize;
    fn fat32StatFd(fd: *mut OpenFile) -> usize;
    fn fat32Getdents64(fd: *mut OpenFile, buf: *mut c_void, lim: u32) -> usize;
}

//
// FAT32 mount
//

#[no_mangle]
pub unsafe extern "C" fn fat32Mount(mount: *mut MountPoint) -> bool {
    (*mount).handlers = &fat32Handlers;
    (*mount).stat = fat32Stat;
    (*mount).lstat = fat32Stat;

    (*mount).fsInfo = malloc(core::mem::size_of::<FAT32>()) as *mut c_void;
    memset((*mount).fsInfo, 0, core::mem::size_of::<FAT32>());

    let fat = FAT_PTR((*mount).fsInfo);
    (*fat).offsetBase = (*mount).mbr.lba_first_sector;

    let mut first_sec = [0u8; SECTOR_SIZE];
    getDiskBytes(first_sec.as_mut_ptr(), (*fat).offsetBase, 1);

    memcpy(
        &mut (*fat).bootsec as *mut _ as *mut c_void,
        first_sec.as_ptr() as *const c_void,
        core::mem::size_of::<FAT32BootSector>(),
    );

    if (*fat).bootsec.bytes_per_sector as usize != SECTOR_SIZE {
        debugf(b"[fat32] Unsupported sector size\n\0".as_ptr());
        panic();
    }

    (*fat).offsetFats =
        (*fat).offsetBase + (*fat).bootsec.reserved_sector_count as u32;

    (*fat).offsetClusters =
        (*fat).offsetFats
            + (*fat).bootsec.table_count as u32
                * (*fat).bootsec.extended_section.table_size_32;

    for i in 0..FAT32_CACHE_MAX {
        (*fat).cacheBase[i] = FAT32_CACHE_BAD;
        let sz = LBA_TO_OFFSET((*fat).bootsec.sectors_per_cluster);
        (*fat).cache[i] = malloc(sz);
        memset((*fat).cache[i] as *mut c_void, 0, sz);
    }

    true
}

//
// Open
//

#[no_mangle]
pub unsafe extern "C" fn fat32Open(
    filename: *mut u8,
    flags: i32,
    _mode: i32,
    fd: *mut OpenFile,
    _symlinkResolve: *mut *mut u8,
) -> usize {
    const O_CREAT: i32 = 0x40;
    const O_WRONLY: i32 = 0x1;
    const O_RDWR: i32 = 0x2;

    if flags & (O_CREAT | O_WRONLY | O_RDWR) != 0 {
        return ERR(30); // EROFS
    }

    let fat = FAT_PTR((*(*fd).mountPoint).fsInfo);

    let res = fat32TraversePath(
        fat,
        filename,
        (*fat).bootsec.extended_section.root_cluster,
    );

    if res.directory == 0 {
        return ERR(2); // ENOENT
    }

    (*fd).dir = malloc(core::mem::size_of::<FAT32OpenFd>()) as *mut c_void;
    memset((*fd).dir, 0, core::mem::size_of::<FAT32OpenFd>());

    let dir = FAT_DIR_PTR((*fd).dir);
    (*dir).ptr = 0;
    (*dir).index = res.index;
    (*dir).directoryStarting = res.directory;
    (*dir).directoryCurr =
        FAT_COMB_HIGH_LOW(res.dirEntry.clusterhigh, res.dirEntry.clusterlow);

    memcpy(
        &mut (*dir).dirEnt as *mut _ as *mut c_void,
        &res.dirEntry as *const _ as *const c_void,
        core::mem::size_of::<FAT32DirectoryEntry>(),
    );

    if res.dirEntry.attrib & FAT_ATTRIB_DIRECTORY != 0 {
        let len = strlength(filename) + 1;
        (*fd).dirname = malloc(len);
        memcpy((*fd).dirname as *mut c_void, filename as *const c_void, len);
    }

    0
}

//
// Read
//

#[no_mangle]
pub unsafe extern "C" fn fat32Read(
    fd: *mut OpenFile,
    buff: *mut u8,
    limit: usize,
) -> usize {
    let fat = FAT_PTR((*(*fd).mountPoint).fsInfo);
    let dir = FAT_DIR_PTR((*fd).dir);

    if (*dir).dirEnt.attrib & FAT_ATTRIB_DIRECTORY != 0 {
        return 0;
    }

    let bytes_per_cluster = LBA_TO_OFFSET((*fat).bootsec.sectors_per_cluster);
    let mut curr = 0usize;

    let tmp = malloc(bytes_per_cluster);
    let needed = DivRoundUp(limit, bytes_per_cluster) as i32;
    let chain = fat32FATchain(fat, (*dir).directoryCurr, needed);

    let mut consec_start = -1;
    let mut consec_end = 0;

    for i in 0..(needed + 1) {
        let cl = *chain.add(i as usize);
        if cl == 0 {
            break;
        }

        (*dir).directoryCurr = cl;

        let last = i == needed - 1;

        if consec_start < 0 {
            if !last && *chain.add((i + 1) as usize) == cl + 1 {
                consec_start = i;
                continue;
            }
        } else {
            if last || *chain.add((i + 1) as usize) != cl + 1 {
                consec_end = i;
            } else {
                continue;
            }
        }

        let offset = (*dir).ptr % bytes_per_cluster;

        if consec_end != 0 {
            let needed_clusters = (consec_end - consec_start + 1) as usize;
            let size = needed_clusters * bytes_per_cluster;
            let opt = malloc(size);

            getDiskBytes(
                opt,
                fat32ClusterToLBA(fat, *chain.add(consec_start as usize)),
                (needed_clusters * (*fat).bootsec.sectors_per_cluster as usize)
                    as u32,
            );

            for j in offset..size {
                if curr >= limit || (*dir).ptr >= (*dir).dirEnt.filesize as usize {
                    free(opt as *mut c_void);
                    goto_cleanup!();
                }
                if !buff.is_null() {
                    *buff.add(curr) = *opt.add(j);
                }
                (*dir).ptr += 1;
                curr += 1;
            }

            free(opt as *mut c_void);
        } else {
            getDiskBytes(
                tmp,
                fat32ClusterToLBA(fat, cl),
                (*fat).bootsec.sectors_per_cluster as u32,
            );

            for j in offset..bytes_per_cluster {
                if curr >= limit || (*dir).ptr >= (*dir).dirEnt.filesize as usize {
                    goto_cleanup!();
                }
                if !buff.is_null() {
                    *buff.add(curr) = *tmp.add(j);
                }
                (*dir).ptr += 1;
                curr += 1;
            }
        }

        consec_start = -1;
        consec_end = 0;
    }

cleanup:
    free(tmp as *mut c_void);
    free(chain as *mut c_void);
    curr
}

//
// Seek / filesize / close / duplicate
//

#[no_mangle]
pub unsafe extern "C" fn fat32Seek(
    fd: *mut OpenFile,
    mut target: usize,
    _offset: isize,
    whence: i32,
) -> usize {
    let fat = FAT_PTR((*(*fd).mountPoint).fsInfo);
    let dir = FAT_DIR_PTR((*fd).dir);

    const SEEK_CURR: i32 = 1;

    if whence == SEEK_CURR {
        target += (*dir).ptr;
    }

    if (*dir).dirEnt.attrib & FAT_ATTRIB_DIRECTORY != 0 {
        return ERR(22);
    }

    if target > (*dir).dirEnt.filesize as usize {
        return ERR(22);
    }

    let old = (*dir).ptr;
    if old > target {
        (*dir).directoryCurr =
            FAT_COMB_HIGH_LOW((*dir).dirEnt.clusterhigh, (*dir).dirEnt.clusterlow);
    }

    (*dir).ptr = target;

    let skip_to = target / LBA_TO_OFFSET((*fat).bootsec.sectors_per_cluster);
    let skip_old = old / LBA_TO_OFFSET((*fat).bootsec.sectors_per_cluster);

    for _ in 0..(skip_to - skip_old) {
        (*dir).directoryCurr = fat32FATtraverse(fat, (*dir).directoryCurr);
        if (*dir).directoryCurr == 0 {
            break;
        }
    }

    (*dir).ptr
}

#[no_mangle]
pub unsafe extern "C" fn fat32GetFilesize(fd: *mut OpenFile) -> usize {
    let dir = FAT_DIR_PTR((*fd).dir);
    if (*dir).dirEnt.attrib & FAT_ATTRIB_DIRECTORY != 0 {
        0
    } else {
        (*dir).dirEnt.filesize as usize
    }
}

#[no_mangle]
pub unsafe extern "C" fn fat32Close(fd: *mut OpenFile) -> bool {
    let dir = FAT_DIR_PTR((*fd).dir);
    if (*dir).dirEnt.attrib & FAT_ATTRIB_DIRECTORY != 0 && !(*fd).dirname.is_null()
    {
        free((*fd).dirname as *mut c_void);
    }
    free((*fd).dir);
    true
}

#[no_mangle]
pub unsafe extern "C" fn fat32DuplicateNodeUnsafe(
    original: *mut OpenFile,
    orphan: *mut OpenFile,
) -> bool {
    (*orphan).dir = malloc(core::mem::size_of::<FAT32OpenFd>()) as *mut c_void;
    memcpy(
        (*orphan).dir,
        (*original).dir,
        core::mem::size_of::<FAT32OpenFd>(),
    );

    if !(*original).dirname.is_null() {
        let len = strlength((*original).dirname) + 1;
        (*orphan).dirname = malloc(len);
        memcpy(
            (*orphan).dirname as *mut c_void,
            (*original).dirname as *const c_void,
            len,
        );
    }

    true
}

//
// Handlers table
//

#[no_mangle]
pub static fat32Handlers: VfsHandlers = VfsHandlers {
    open: fat32Open,
    close: fat32Close,
    duplicate: fat32DuplicateNodeUnsafe,
    read: fat32Read,
    stat: fat32StatFd,
    getdents64: fat32Getdents64,
    seek: fat32Seek,
    getFilesize: fat32GetFilesize,
};
