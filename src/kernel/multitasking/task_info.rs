#![no_std]

use core::ffi::c_void;
use core::ptr;

//
// Constants
//

const USER_HEAP_START: usize = 0x0000_4000_0000;
const USER_MMAP_START: usize = 0x0000_6000_0000;

const S_IWGRP: u32 = 0o020;
const S_IWOTH: u32 = 0o002;

//
// Externals
//

extern "C" {
    fn calloc(nmemb: usize, size: usize) -> *mut c_void;
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);

    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);
    fn strlength(s: *const u8) -> usize;

    fn spinlockAcquire(lock: *mut c_void);
    fn spinlockRelease(lock: *mut c_void);

    fn spinlockCntWriteAcquire(lock: *mut SpinlockCnt);
    fn spinlockCntWriteRelease(lock: *mut SpinlockCnt);

    fn PageDirectoryAllocate() -> *mut c_void;
    fn PageDirectoryUserDuplicate(src: *mut c_void, dst: *mut c_void);
    fn PageDirectoryFree(pd: *mut c_void);

    fn fsUserClose(task: *mut c_void, fd: i32);
}

//
// Locks
//

#[repr(C)]
pub struct SpinlockCnt {
    pub value: i32,
}

//
// TaskInfo structs
//

#[repr(C)]
pub struct TaskInfoFs {
    pub utilizedBy: u32,
    pub cwd: *mut u8,
    pub umask: u32,
    pub LOCK_FS: *mut c_void,
}

#[repr(C)]
pub struct TaskInfoPagedir {
    pub utilizedBy: u32,
    pub pagedir: *mut c_void,

    pub heap_start: usize,
    pub heap_end: usize,

    pub mmap_start: usize,
    pub mmap_end: usize,

    pub LOCK_PD: *mut c_void,
}

#[repr(C)]
pub struct TaskInfoFiles {
    pub utilizedBy: u32,
    pub rlimitFdsHard: usize,
    pub rlimitFdsSoft: usize,
    pub fdBitmap: *mut u8,

    pub firstFile: *mut FileNode,
    pub WLOCK_FILES: SpinlockCnt,
}

#[repr(C)]
pub struct TaskInfoSignal {
    pub utilizedBy: u32,
    pub signals: [u64; 64],
    pub LOCK_SIGNAL: *mut c_void,
}

#[repr(C)]
pub struct FileNode {
    pub key: i32,
    pub next: *mut FileNode,
}

//
// CLONE_FS
//

#[no_mangle]
pub unsafe extern "C" fn taskInfoFsAllocate() -> *mut TaskInfoFs {
    let target = calloc(1, core::mem::size_of::<TaskInfoFs>()) as *mut TaskInfoFs;
    (*target).utilizedBy = 1;

    (*target).cwd = calloc(2, 1) as *mut u8;
    *(*target).cwd = b'/';

    (*target).umask = S_IWGRP | S_IWOTH;
    target
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoFsDiscard(target: *mut TaskInfoFs) {
    spinlockAcquire((*target).LOCK_FS);
    (*target).utilizedBy -= 1;

    if (*target).utilizedBy == 0 {
        free((*target).cwd as *mut c_void);
        free(target as *mut c_void);
    } else {
        spinlockRelease((*target).LOCK_FS);
    }
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoFsClone(old: *mut TaskInfoFs) -> *mut TaskInfoFs {
    let new = taskInfoFsAllocate();

    spinlockAcquire((*old).LOCK_FS);

    (*new).umask = (*old).umask;

    let len = strlength((*old).cwd) + 1;
    free((*new).cwd as *mut c_void);

    (*new).cwd = malloc(len) as *mut u8;
    memcpy((*new).cwd as *mut c_void, (*old).cwd as *const c_void, len);

    spinlockRelease((*old).LOCK_FS);
    new
}

//
// CLONE_VM
//

#[no_mangle]
pub unsafe extern "C" fn taskInfoPdAllocate(pagedir: bool) -> *mut TaskInfoPagedir {
    let target =
        calloc(1, core::mem::size_of::<TaskInfoPagedir>()) as *mut TaskInfoPagedir;

    (*target).utilizedBy = 1;

    if pagedir {
        (*target).pagedir = PageDirectoryAllocate();
    }

    (*target).heap_start = USER_HEAP_START;
    (*target).heap_end = USER_HEAP_START;

    (*target).mmap_start = USER_MMAP_START;
    (*target).mmap_end = USER_MMAP_START;

    target
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoPdClone(old: *mut TaskInfoPagedir) -> *mut TaskInfoPagedir {
    let new = taskInfoPdAllocate(true);

    spinlockAcquire((*old).LOCK_PD);

    PageDirectoryUserDuplicate((*old).pagedir, (*new).pagedir);

    (*new).heap_start = (*old).heap_start;
    (*new).heap_end = (*old).heap_end;

    (*new).mmap_start = (*old).mmap_start;
    (*new).mmap_end = (*old).mmap_end;

    spinlockRelease((*old).LOCK_PD);
    new
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoPdDiscard(target: *mut TaskInfoPagedir) {
    spinlockAcquire((*target).LOCK_PD);
    (*target).utilizedBy -= 1;

    if (*target).utilizedBy == 0 {
        PageDirectoryFree((*target).pagedir);
        // intentionally leaked (scheduler safety)
    } else {
        spinlockRelease((*target).LOCK_PD);
    }
}

//
// CLONE_FILES
//

#[no_mangle]
pub unsafe extern "C" fn taskInfoFilesAllocate() -> *mut TaskInfoFiles {
    let target =
        calloc(1, core::mem::size_of::<TaskInfoFiles>()) as *mut TaskInfoFiles;

    (*target).utilizedBy = 1;
    (*target).rlimitFdsHard = 1024;
    (*target).rlimitFdsSoft = 1024;

    (*target).fdBitmap = calloc((*target).rlimitFdsHard / 8, 1) as *mut u8;
    target
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoFilesDiscard(
    target: *mut TaskInfoFiles,
    task: *mut c_void,
) {
    spinlockCntWriteAcquire(&mut (*target).WLOCK_FILES);
    (*target).utilizedBy -= 1;

    if (*target).utilizedBy == 0 {
        spinlockCntWriteRelease(&mut (*target).WLOCK_FILES);

        while !(*target).firstFile.is_null() {
            fsUserClose(task, (*(*target).firstFile).key);
        }

        free((*target).fdBitmap as *mut c_void);
        free(target as *mut c_void);
    } else {
        spinlockCntWriteRelease(&mut (*target).WLOCK_FILES);
    }
}

//
// SIGNALS
//

#[no_mangle]
pub unsafe extern "C" fn taskInfoSignalAllocate() -> *mut TaskInfoSignal {
    let target =
        calloc(1, core::mem::size_of::<TaskInfoSignal>()) as *mut TaskInfoSignal;
    (*target).utilizedBy = 1;
    target
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoSignalClone(old: *mut TaskInfoSignal) -> *mut TaskInfoSignal {
    let target = taskInfoSignalAllocate();

    spinlockAcquire((*old).LOCK_SIGNAL);
    memcpy(
        target as *mut c_void,
        old as *const c_void,
        core::mem::size_of::<TaskInfoSignal>(),
    );
    spinlockRelease((*old).LOCK_SIGNAL);

    target
}

#[no_mangle]
pub unsafe extern "C" fn taskInfoSignalDiscard(target: *mut TaskInfoSignal) {
    spinlockAcquire((*target).LOCK_SIGNAL);
    (*target).utilizedBy -= 1;

    if (*target).utilizedBy == 0 {
        free(target as *mut c_void);
    } else {
        spinlockRelease((*target).LOCK_SIGNAL);
    }
}
