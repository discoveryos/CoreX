#![no_std]

use core::cmp::min;
use core::ptr::{copy_nonoverlapping, null_mut};
use core::sync::atomic::{AtomicU64, Ordering};

//
// Constants
//

const MAX_EVENTS: usize = 64;
const EVENT_BUFFER_SIZE: usize = 4096;

const O_NONBLOCK: u32 = 0x800;
const EPOLLIN: i32 = 0x001;

const EWOULDBLOCK: isize = 11;
const EINTR: isize = 4;
const ENOTTY: isize = 25;
const EINVAL: isize = 22;
const ENOENT: isize = 2;

const EV_CNT: usize = 32;
const ABS_CNT: usize = 64;

//
// Error helper
//

#[inline]
const fn err(code: isize) -> usize {
    (!code + 1) as usize
}

//
// External kernel symbols (stubs)
//

extern "C" {
    static mut timerTicks: u64;
    static mut currentTask: *mut Task;

    fn signalsPendingQuick(task: *mut Task) -> bool;

    fn fakefsAddFile(
        root: *mut FakeFsNode,
        dir: *mut FakeFsNode,
        name: *const u8,
        uid: u32,
        mode: u32,
        handlers: *const VfsHandlers,
    );

    fn fakefsFstat();

    fn panic() -> !;
    fn debugf(fmt: *const u8, ...) -> ();
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
}

//
// Circular buffer
//

#[repr(C)]
pub struct CircularInt {
    _priv: u32,
}

extern "C" {
    fn CircularIntAllocate(buf: *mut CircularInt, size: usize);
    fn CircularIntWrite(buf: *mut CircularInt, data: *const u8, size: usize) -> usize;
    fn CircularIntRead(buf: *mut CircularInt, out: *mut u8, size: usize) -> usize;
    fn CircularIntReadPoll(buf: *mut CircularInt) -> usize;
}

//
// Input structs (Linux ABI)
//

#[repr(C)]
pub struct input_event {
    pub sec: u64,
    pub usec: u64,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
pub struct input_id {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

//
// VFS
//

#[repr(C)]
pub struct OpenFile {
    pub flags: u32,
    pub dir: *mut DevInputEvent,
}

#[repr(C)]
pub struct VfsHandlers {
    pub open: Option<extern "C" fn(*mut u8, i32, i32, *mut OpenFile, *mut *mut u8) -> usize>,
    pub read: Option<extern "C" fn(*mut OpenFile, *mut u8, usize) -> usize>,
    pub ioctl: Option<extern "C" fn(*mut OpenFile, u64, *mut u8) -> usize>,
    pub internalPoll: Option<extern "C" fn(*mut OpenFile, i32) -> i32>,
    pub stat: Option<extern "C" fn()>,
    pub duplicate: Option<extern "C" fn(*mut OpenFile, *mut OpenFile) -> bool>,
    pub reportKey: Option<extern "C" fn(*mut OpenFile) -> usize>,
    pub close: Option<extern "C" fn(*mut OpenFile) -> bool>,
}

//
// Device
//

pub type EventBitFn =
    extern "C" fn(*mut OpenFile, u64, *mut u8) -> usize;

#[repr(C)]
pub struct DevInputEvent {
    pub timesOpened: AtomicU64,
    pub LOCK_USERSPACE: Spinlock,

    pub deviceEvents: CircularInt,

    pub inputid: input_id,
    pub devname: *mut u8,
    pub physloc: *mut u8,
    pub properties: usize,

    pub eventBit: Option<EventBitFn>,
}

//
// Globals
//

static mut devInputEvents: [DevInputEvent; MAX_EVENTS] =
    unsafe { core::mem::zeroed() };

static mut lastInputEvent: usize = 0;

extern "C" {
    static mut rootDev: *mut FakeFsNode;
    static mut inputFakedir: *mut FakeFsNode;
}

//
// Helpers
//

#[inline]
unsafe fn ioctl_write(dst: *mut u8, val: usize, size: usize) {
    let to_copy = min(core::mem::size_of::<usize>(), size);
    copy_nonoverlapping(&val as *const usize as *const u8, dst, to_copy);
}

//
// Kernel → userspace event injection
//

#[no_mangle]
pub unsafe extern "C" fn inputGenerateEvent(
    item: *mut DevInputEvent,
    type_: u16,
    code: u16,
    value: i32,
) {
    if (*item).timesOpened.load(Ordering::Relaxed) == 0 {
        return;
    }

    let mut ev: input_event = core::mem::zeroed();
    ev.sec = timerTicks / 1000;
    ev.usec = (timerTicks % 1000) * 1000;
    ev.type_ = type_;
    ev.code = code;
    ev.value = value;

    let written = CircularIntWrite(
        &mut (*item).deviceEvents,
        &ev as *const _ as *const u8,
        core::mem::size_of::<input_event>(),
    );

    if written != core::mem::size_of::<input_event>() {
        panic();
    }
}

//
// File operations
//

#[no_mangle]
pub unsafe extern "C" fn devInputEventOpen(
    filename: *mut u8,
    _flags: i32,
    _mode: i32,
    fd: *mut OpenFile,
    _symlink: *mut *mut u8,
) -> usize {
    let num = numAtEnd(filename) as usize;
    if num >= lastInputEvent {
        return err(ENOENT);
    }

    let event = &mut devInputEvents[num];
    spinlockAcquire(&mut event.LOCK_USERSPACE);
    event.timesOpened.fetch_add(1, Ordering::Relaxed);
    (*fd).dir = event;
    spinlockRelease(&mut event.LOCK_USERSPACE);
    0
}

#[no_mangle]
pub unsafe extern "C" fn devInputEventRead(
    fd: *mut OpenFile,
    out: *mut u8,
    limit: usize,
) -> usize {
    let event = (*fd).dir;

    loop {
        let cnt = CircularIntRead(&mut (*event).deviceEvents, out, limit);
        if cnt > 0 {
            return cnt;
        }

        if (*fd).flags & O_NONBLOCK != 0 {
            return err(EWOULDBLOCK);
        }

        if signalsPendingQuick(currentTask) {
            return err(EINTR);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn devInputEventIoctl(
    fd: *mut OpenFile,
    request: u64,
    arg: *mut u8,
) -> usize {
    let event = (*fd).dir;
    let number = (request & 0xff) as usize;
    let size = ((request >> 16) & 0x3fff) as usize;

    if let Some(bitfn) = (*event).eventBit {
        if (0x20..0x20 + EV_CNT).contains(&number)
            || (0x40..0x40 + ABS_CNT).contains(&number)
        {
            return bitfn(fd, request, arg);
        }
    }

    match number {
        0x01 => {
            *(arg as *mut i32) = 0x10001;
            0
        }
        0x02 => {
            copy_nonoverlapping(
                &(*event).inputid as *const _ as *const u8,
                arg,
                core::mem::size_of::<input_id>(),
            );
            0
        }
        0x06 => {
            let len = c_strlen((*event).devname) + 1;
            let to_copy = min(size, len);
            copy_nonoverlapping((*event).devname, arg, to_copy);
            to_copy
        }
        0x07 => {
            let len = c_strlen((*event).physloc) + 1;
            let to_copy = min(size, len);
            copy_nonoverlapping((*event).physloc, arg, to_copy);
            to_copy
        }
        0x08 => err(ENOENT),
        0x09 => {
            ioctl_write(arg, (*event).properties, size);
            size
        }
        0x18 | 0x19 | 0x1b => {
            if let Some(bitfn) = (*event).eventBit {
                bitfn(fd, request, arg)
            } else {
                err(ENOTTY)
            }
        }
        _ => err(ENOTTY),
    }
}

#[no_mangle]
pub unsafe extern "C" fn devInputInternalPoll(fd: *mut OpenFile, events: i32) -> i32 {
    let event = (*fd).dir;
    let cnt = CircularIntReadPoll(&mut (*event).deviceEvents);
    if cnt > 0 && (events & EPOLLIN) != 0 {
        EPOLLIN
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn devInputReportKey(fd: *mut OpenFile) -> usize {
    (*fd).dir as usize
}

#[no_mangle]
pub unsafe extern "C" fn devInputEventDuplicate(
    original: *mut OpenFile,
    orphan: *mut OpenFile,
) -> bool {
    (*orphan).dir = (*original).dir;
    let event = (*original).dir;

    spinlockAcquire(&mut (*event).LOCK_USERSPACE);
    (*event).timesOpened.fetch_add(1, Ordering::Relaxed);
    spinlockRelease(&mut (*event).LOCK_USERSPACE);
    true
}

#[no_mangle]
pub unsafe extern "C" fn devInputEventClose(fd: *mut OpenFile) -> bool {
    let event = (*fd).dir;
    spinlockAcquire(&mut (*event).LOCK_USERSPACE);
    (*event).timesOpened.fetch_sub(1, Ordering::Relaxed);
    spinlockRelease(&mut (*event).LOCK_USERSPACE);
    true
}

//
// Registration
//

#[no_mangle]
pub static devInputEventHandlers: VfsHandlers = VfsHandlers {
    open: Some(devInputEventOpen),
    read: Some(devInputEventRead),
    ioctl: Some(devInputEventIoctl),
    internalPoll: Some(devInputInternalPoll),
    stat: Some(fakefsFstat),
    duplicate: Some(devInputEventDuplicate),
    reportKey: Some(devInputReportKey),
    close: Some(devInputEventClose),
};

//
// Setup
//

#[no_mangle]
pub unsafe extern "C" fn devInputEventSetup(devname: *const u8) -> *mut DevInputEvent {
    if lastInputEvent >= MAX_EVENTS {
        panic();
    }

    let idx = lastInputEvent;
    let item = &mut devInputEvents[idx];

    item.devname = strdup(devname);
    item.physloc = strdup(b"serio1\0".as_ptr());
    CircularIntAllocate(&mut item.deviceEvents, EVENT_BUFFER_SIZE);

    fakefsAddFile(
        rootDev,
        inputFakedir,
        format_event_name(idx),
        0,
        0o100600,
        &devInputEventHandlers,
    );

    lastInputEvent += 1;
    item
}
