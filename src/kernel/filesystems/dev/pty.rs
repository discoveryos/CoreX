#![no_std]

use core::cmp::min;
use core::ptr::{copy_nonoverlapping, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

//
// Constants
//

const PTY_MAX: usize = 256;
const PTY_BUFF_SIZE: usize = 4096;

const O_NONBLOCK: u32 = 0x800;

const EPOLLIN: i32 = 0x001;
const EPOLLOUT: i32 = 0x004;

const EWOULDBLOCK: isize = 11;
const ENOENT: isize = 2;
const EIO: isize = 5;
const EPERM: isize = 1;
const ENOTTY: isize = 25;

#[inline]
const fn err(code: isize) -> usize {
    (!code + 1) as usize
}

//
// Kernel primitives
//

#[repr(C)]
pub struct Spinlock {
    _priv: u32,
}

extern "C" {
    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    fn spinlockCntReadAcquire(lock: *mut Spinlock);
    fn spinlockCntReadRelease(lock: *mut Spinlock);
}

//
// External kernel APIs
//

extern "C" {
    fn bitmapGenericGet(map: *mut u8, idx: usize) -> bool;
    fn bitmapGenericSet(map: *mut u8, idx: usize, val: bool);

    fn calloc(n: usize, size: usize) -> *mut u8;
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);

    fn memcpy(dst: *mut u8, src: *const u8, n: usize);
    fn memmove(dst: *mut u8, src: *const u8, n: usize);

    fn handControl();

    fn pollInstanceRing(key: usize, events: i32);

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;
}

//
// Linked list
//

#[repr(C)]
pub struct LLcontrol {
    _priv: u32,
}

extern "C" {
    fn LinkedListInit(ctrl: *mut LLcontrol, size: usize);
    fn LinkedListAllocate(ctrl: *mut LLcontrol, size: usize) -> *mut PtyPair;
    fn LinkedListRemove(ctrl: *mut LLcontrol, size: usize, elem: *mut PtyPair) -> bool;
    fn LinkedListSearch(
        ctrl: *mut LLcontrol,
        cb: extern "C" fn(*mut u8, *mut u8) -> bool,
        ctx: *mut u8,
    ) -> *mut PtyPair;
}

//
// Tasks
//

#[repr(C)]
pub struct Task {
    pub next: *mut Task,
    pub state: i32,
    pub ctrlPty: i32,
    pub sid: i32,
    pub tgid: i32,
    pub pgid: i32,
}

extern "C" {
    static mut firstTask: *mut Task;
    static mut currentTask: *mut Task;
    static mut TASK_LL_MODIFY: Spinlock;
}

const TASK_STATE_DEAD: i32 = 3;

//
// VFS
//

#[repr(C)]
pub struct OpenFile {
    pub flags: u32,
    pub dir: *mut PtyPair,
}

#[repr(C)]
pub struct VfsHandlers {
    pub open: Option<extern "C" fn(*mut u8, i32, i32, *mut OpenFile, *mut *mut u8) -> usize>,
    pub duplicate: Option<extern "C" fn(*mut OpenFile, *mut OpenFile) -> bool>,
    pub close: Option<extern "C" fn(*mut OpenFile) -> bool>,
    pub read: Option<extern "C" fn(*mut OpenFile, *mut u8, usize) -> usize>,
    pub write: Option<extern "C" fn(*mut OpenFile, *mut u8, usize) -> usize>,
    pub internalPoll: Option<extern "C" fn(*mut OpenFile, i32) -> i32>,
    pub ioctl: Option<extern "C" fn(*mut OpenFile, u64, *mut u8) -> usize>,
    pub reportKey: Option<extern "C" fn(*mut OpenFile) -> usize>,
    pub stat: Option<extern "C" fn()>,
}

extern "C" {
    fn fakefsFstat();
}

//
// termios / winsize
//

#[repr(C)]
pub struct winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[repr(C)]
pub struct termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_cc: [u8; 32],
}

//
// Flags (subset)
//

const ICRNL: u32 = 0x100;
const IXON: u32 = 0x400;
const BRKINT: u32 = 0x2;
const ISTRIP: u32 = 0x20;
const INPCK: u32 = 0x10;

const OPOST: u32 = 0x1;
const ONLCR: u32 = 0x4;

const ISIG: u32 = 0x1;
const ICANON: u32 = 0x2;
const ECHO: u32 = 0x8;
const ECHOE: u32 = 0x10;
const ECHOK: u32 = 0x20;

//
// Pty pair
//

#[repr(C)]
pub struct PtyPair {
    pub LOCK_PTY: Spinlock,

    pub id: i32,
    pub locked: bool,

    pub masterFds: usize,
    pub slaveFds: usize,

    pub bufferMaster: *mut u8,
    pub bufferSlave: *mut u8,

    pub ptrMaster: usize,
    pub ptrSlave: usize,

    pub term: termios,
    pub win: winsize,

    pub ctrlSession: i32,
    pub ctrlPgid: i32,
}

//
// Globals
//

static mut ptyBitmap: *mut u8 = null_mut();
static mut LOCK_PTY_GLOBAL: Spinlock = Spinlock { _priv: 0 };
static mut dsPtyPair: LLcontrol = LLcontrol { _priv: 0 };

//
// Bitmap helpers
//

unsafe fn ptyBitmapDecide() -> i32 {
    let mut ret = -1;
    spinlockAcquire(&mut LOCK_PTY_GLOBAL);
    for i in 0..PTY_MAX {
        if !bitmapGenericGet(ptyBitmap, i) {
            bitmapGenericSet(ptyBitmap, i, true);
            ret = i as i32;
            break;
        }
    }
    spinlockRelease(&mut LOCK_PTY_GLOBAL);
    if ret == -1 {
        panic();
    }
    ret
}

unsafe fn ptyBitmapRemove(idx: i32) {
    spinlockAcquire(&mut LOCK_PTY_GLOBAL);
    bitmapGenericSet(ptyBitmap, idx as usize, false);
    spinlockRelease(&mut LOCK_PTY_GLOBAL);
}

//
// Init
//

#[no_mangle]
pub unsafe extern "C" fn initiatePtyInterface() {
    LinkedListInit(&mut dsPtyPair, core::mem::size_of::<PtyPair>());
    ptyBitmap = calloc(PTY_MAX / 8, 1);
}

//
// /dev/ptmx handlers
//

#[no_mangle]
pub unsafe extern "C" fn ptmxOpen(
    _filename: *mut u8,
    _flags: i32,
    _mode: i32,
    fd: *mut OpenFile,
    _sym: *mut *mut u8,
) -> usize {
    let id = ptyBitmapDecide();
    spinlockAcquire(&mut LOCK_PTY_GLOBAL);
    let pair = LinkedListAllocate(&mut dsPtyPair, core::mem::size_of::<PtyPair>());
    (*pair).id = id;
    (*pair).masterFds = 1;
    (*pair).bufferMaster = malloc(PTY_BUFF_SIZE);
    (*pair).bufferSlave = malloc(PTY_BUFF_SIZE);
    (*pair).win.ws_row = 24;
    (*pair).win.ws_col = 80;
    (*fd).dir = pair;
    spinlockRelease(&mut LOCK_PTY_GLOBAL);
    0
}

#[no_mangle]
pub unsafe extern "C" fn ptmxDuplicate(orig: *mut OpenFile, new: *mut OpenFile) -> bool {
    (*new).dir = (*orig).dir;
    let pair = (*orig).dir;
    spinlockAcquire(&mut (*pair).LOCK_PTY);
    (*pair).masterFds += 1;
    spinlockRelease(&mut (*pair).LOCK_PTY);
    true
}

#[no_mangle]
pub unsafe extern "C" fn ptmxClose(fd: *mut OpenFile) -> bool {
    let pair = (*fd).dir;
    spinlockAcquire(&mut (*pair).LOCK_PTY);
    (*pair).masterFds -= 1;
    if (*pair).masterFds == 0 && (*pair).slaveFds == 0 {
        spinlockRelease(&mut (*pair).LOCK_PTY);
        free((*pair).bufferMaster);
        free((*pair).bufferSlave);
        ptyBitmapRemove((*pair).id);
        spinlockAcquire(&mut LOCK_PTY_GLOBAL);
        LinkedListRemove(&mut dsPtyPair, core::mem::size_of::<PtyPair>(), pair);
        spinlockRelease(&mut LOCK_PTY_GLOBAL);
    } else {
        spinlockRelease(&mut (*pair).LOCK_PTY);
    }
    true
}

//
// Registration
//

#[no_mangle]
pub static handlePtmx: VfsHandlers = VfsHandlers {
    open: Some(ptmxOpen),
    duplicate: Some(ptmxDuplicate),
    close: Some(ptmxClose),
    read: None,
    write: None,
    internalPoll: None,
    ioctl: None,
    reportKey: None,
    stat: None,
};
