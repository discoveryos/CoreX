#![no_std]

use core::ptr;
use core::ffi::c_void;

// ======== C TYPES / CONSTANTS ========

pub type err_t = i32;
pub type u32_t = u32;
pub type u8_t = u8;
pub type sys_thread_t = u64;

pub const ERR_OK: err_t = 0;
pub const ERR_MEM: err_t = -1;
pub const SYS_ARCH_TIMEOUT: u32_t = 0xFFFFFFFF;
pub const SYS_MBOX_EMPTY: u32_t = 0xFFFFFFFF;

// ======== EXTERNAL KERNEL SYMBOLS ========

extern "C" {
    // locks
    fn spinlockAcquire(lock: *mut Spinlock);
    fn spinlockRelease(lock: *mut Spinlock);

    // semaphores
    fn semaphorePost(sem: *mut sys_sem_t);
    fn semaphoreWait(sem: *mut sys_sem_t, timeout: u32_t) -> bool;

    // tasks / scheduler
    static mut currentTask: *mut Task;
    fn handControl();
    fn taskSpinlockExit(task: *mut Task, lock: *mut Spinlock);

    // task creation
    fn taskCreateKernel(entry: u64, arg: u64) -> *mut Task;
    fn taskNameKernel(task: *mut Task, name: *const u8, len: usize);

    // memory
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);

    // misc
    fn panic() -> !;
    fn debugf(fmt: *const u8, ...) -> i32;

    static timerBootUnix: u64;
    static timerTicks: u64;

    static lwipCmdline: [u8; 128];
}

// ======== STRUCTS ========

#[repr(C)]
pub struct Spinlock {
    _priv: u64,
}

#[repr(C)]
pub struct sys_sem_t {
    pub invalid: bool,
    pub cnt: u32,
    pub LOCK: Spinlock,
}

#[repr(C)]
pub struct mboxBlock {
    pub task: *mut Task,
    pub write: bool,
    pub _ll: LinkedListNode,
}

#[repr(C)]
pub struct sys_mbox_t {
    pub LOCK: Spinlock,
    pub invalid: bool,
    pub size: usize,
    pub msges: *mut *mut c_void,
    pub ptrRead: usize,
    pub ptrWrite: usize,
    pub firstBlock: LinkedList,
}

#[repr(C)]
pub struct LinkedList {
    pub firstObject: *mut c_void,
}

#[repr(C)]
pub struct LinkedListNode {
    pub next: *mut c_void,
}

#[repr(C)]
pub struct Task {
    pub id: u64,
    pub state: u32,
    pub forcefulWakeupTimeUnsafe: u64,
    pub lwipSem: sys_sem_t,
}

// ======== LINKED LIST HELPERS ========

extern "C" {
    fn LinkedListInit(list: *mut LinkedList, obj_size: usize);
    fn LinkedListAllocate(list: *mut LinkedList, obj_size: usize) -> *mut mboxBlock;
    fn LinkedListRemove(list: *mut LinkedList, obj_size: usize, obj: *mut mboxBlock);
}

// ======== LWIP SYS API ========

#[no_mangle]
pub extern "C" fn sys_init() {}

#[no_mangle]
pub extern "C" fn sys_mutex_lock(lock: *mut Spinlock) {
    unsafe { spinlockAcquire(lock) }
}

#[no_mangle]
pub extern "C" fn sys_mutex_unlock(lock: *mut Spinlock) {
    unsafe { spinlockRelease(lock) }
}

#[no_mangle]
pub extern "C" fn sys_mutex_new(lock: *mut Spinlock) -> err_t {
    unsafe {
        ptr::write_bytes(lock, 0, 1);
    }
    ERR_OK
}

#[no_mangle]
pub extern "C" fn sys_sem_new(sem: *mut sys_sem_t, cnt: u8) -> err_t {
    unsafe {
        if cnt != 0 {
            debugf(b"[lwip::glue::sem::new] cnt{%d}\n\0".as_ptr(), cnt);
            panic();
        }
        (*sem).invalid = false;
        (*sem).cnt = 0;
        sys_mutex_new(&mut (*sem).LOCK);
    }
    ERR_OK
}

#[no_mangle]
pub extern "C" fn LWIP_NETCONN_THREAD_SEM_ALLOC() -> err_t {
    unsafe {
        sys_sem_new(&mut (*currentTask).lwipSem, 0);
    }
    ERR_OK
}

#[no_mangle]
pub extern "C" fn LWIP_NETCONN_THREAD_SEM_FREE() -> err_t {
    unsafe {
        sys_sem_new(&mut (*currentTask).lwipSem, 0);
    }
    ERR_OK
}

#[no_mangle]
pub extern "C" fn LWIP_NETCONN_THREAD_SEM_GET() -> *mut sys_sem_t {
    unsafe { &mut (*currentTask).lwipSem }
}

#[no_mangle]
pub extern "C" fn sys_sem_signal(sem: *mut sys_sem_t) {
    unsafe { semaphorePost(sem) }
}

#[no_mangle]
pub extern "C" fn sys_arch_sem_wait(sem: *mut sys_sem_t, timeout: u32_t) -> u32_t {
    unsafe {
        if timeout != 0 {
            debugf(b"[lwip::glue] todo: Timeout\n\0".as_ptr());
            panic();
        }
        let ok = semaphoreWait(sem, timeout);
        if ok { 0 } else { SYS_ARCH_TIMEOUT }
    }
}

#[no_mangle]
pub extern "C" fn sys_sem_free(sem: *mut sys_sem_t) {
    unsafe { sys_sem_new(sem, 0); }
}

#[no_mangle]
pub extern "C" fn sys_sem_set_invalid(sem: *mut sys_sem_t) {
    unsafe {
        if (*sem).invalid {
            debugf(b"[lwip::glue] Already invalid!\n\0".as_ptr());
            panic();
        }
        (*sem).invalid = true;
    }
}

#[no_mangle]
pub extern "C" fn sys_sem_valid(sem: *mut sys_sem_t) -> i32 {
    unsafe { (!(*sem).invalid) as i32 }
}

#[no_mangle]
pub extern "C" fn sys_now() -> u32_t {
    unsafe { (timerBootUnix * 1000 + timerTicks) as u32_t }
}

#[no_mangle]
pub extern "C" fn sys_thread_new(
    _name: *const u8,
    thread: extern "C" fn(*mut c_void),
    arg: *mut c_void,
    _stack: i32,
    _prio: i32,
) -> sys_thread_t {
    unsafe {
        let task = taskCreateKernel(thread as u64, arg as u64);
        taskNameKernel(task, lwipCmdline.as_ptr(), lwipCmdline.len());
        (*task).id
    }
}
