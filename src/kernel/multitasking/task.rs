#![no_std]

use core::ffi::c_void;
use core::ptr;

//
// Externals, constants, and flags
//

const PAGE_SIZE: usize = 4096;
const BLOCK_SIZE: usize = 4096;
const USER_STACK_PAGES: usize = 8;

const TASK_STATE_DEAD: i32 = 0;
const TASK_STATE_CREATED: i32 = 1;
const TASK_STATE_READY: i32 = 2;
const TASK_STATE_BLOCKED: i32 = 3;
const TASK_STATE_DUMMY: i32 = 4;
const TASK_STATE_WAITING_CHILD: i32 = 5;
const TASK_STATE_WAITING_CHILD_SPECIFIC: i32 = 6;
const TASK_STATE_WAITING_VFORK: i32 = 7;

const GDT_KERNEL_CODE: u64 = 0x08;
const GDT_KERNEL_DATA: u64 = 0x10;
const GDT_USER_CODE: u64 = 0x18;
const GDT_USER_DATA: u64 = 0x20;
const DPL_USER: u64 = 3;

const KERNEL_TASK_ID: u64 = 0;

//
// Externals
//

extern "C" {
    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;

    fn memset(dst: *mut c_void, val: i32, size: usize);
    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);

    fn VirtualAllocate(pages: usize) -> *mut c_void;
    fn PhysicalAllocate(pages: i32) -> usize;

    fn VirtualMap(virt: usize, phys: usize, flags: u64);
    fn VirtualToPhysical(virt: usize) -> usize;

    fn GetPageDirectory() -> *mut c_void;
    fn ChangePageDirectory(pd: *mut c_void);

    fn spinlockAcquire(lock: *mut c_void);
    fn spinlockRelease(lock: *mut c_void);

    fn spinlockCntReadAcquire(lock: *mut SpinlockCnt);
    fn spinlockCntReadRelease(lock: *mut SpinlockCnt);
    fn spinlockCntWriteAcquire(lock: *mut SpinlockCnt);
    fn spinlockCntWriteRelease(lock: *mut SpinlockCnt);

    fn atomicBitmapSet(bitmap: *mut u64, bit: usize);
    fn atomicWrite32(ptr: *mut u32, val: u32);

    fn futexSyscall(
        uaddr: *mut u32,
        op: i32,
        val: i32,
        timeout: usize,
        uaddr2: usize,
        val3: i32,
    );

    fn handControl();

    fn taskInfoPdAllocate(user: bool) -> *mut TaskInfoPagedir;
    fn taskInfoPdClone(src: *mut TaskInfoPagedir) -> *mut TaskInfoPagedir;
    fn taskInfoPdDiscard(pd: *mut TaskInfoPagedir);

    fn taskInfoFsAllocate() -> *mut TaskInfoFs;
    fn taskInfoFsClone(src: *mut TaskInfoFs) -> *mut TaskInfoFs;

    fn taskInfoFilesAllocate() -> *mut TaskInfoFiles;
    fn taskInfoFilesDiscard(info: *mut TaskInfoFiles, task: *mut Task);

    fn taskInfoSignalAllocate() -> *mut TaskInfoSignal;
    fn taskInfoSignalClone(src: *mut TaskInfoSignal) -> *mut TaskInfoSignal;

    fn stackGenerateKernel(task: *mut Task, param: u64);

    fn LinkedListInit(list: *mut LinkedList, elem_size: usize);
    fn LinkedListAllocate(list: *mut LinkedList, elem_size: usize) -> *mut c_void;
    fn LinkedListRemove(list: *mut LinkedList, elem_size: usize, obj: *mut c_void);
}

//
// Synchronization
//

#[repr(C)]
pub struct SpinlockCnt {
    pub value: i32,
}

static mut TASK_LL_MODIFY: SpinlockCnt = SpinlockCnt { value: 0 };

//
// Core task structures (partial)
//

#[repr(C)]
pub struct Registers {
    pub rax: u64,
    pub rdi: u64,
    pub rip: u64,
    pub cs: u64,
    pub ds: u64,
    pub rflags: u64,
    pub usermode_rsp: usize,
    pub usermode_ss: u64,
}

#[repr(C)]
pub struct Task {
    pub id: u64,
    pub tgid: u64,
    pub pgid: u64,
    pub sid: u64,

    pub next: *mut Task,
    pub parent: *mut Task,

    pub kernel_task: bool,
    pub state: i32,
    pub extras: u32,

    pub registers: Registers,

    pub whileTssRsp: u64,
    pub whileSyscallRsp: u64,

    pub infoPd: *mut TaskInfoPagedir,
    pub infoFs: *mut TaskInfoFs,
    pub infoFiles: *mut TaskInfoFiles,
    pub infoSignals: *mut TaskInfoSignal,

    pub cmdline: *mut u8,
    pub cmdlineLen: usize,

    pub fpuenv: [u8; 512],
    pub mxcsr: u32,

    pub spinlockQueueEntry: *mut c_void,

    pub sigPendingList: u64,
    pub sigBlockList: u64,

    pub tidptr: *mut u32,

    pub noInformParent: bool,

    pub dsChildTerminated: LinkedList,
    pub dsSysIntr: LinkedList,
}

#[repr(C)]
pub struct TaskInfoPagedir {
    pub pagedir: *mut c_void,
    pub heap_start: usize,
    pub heap_end: usize,
    pub utilizedBy: u32,
    pub LOCK_PD: *mut c_void,
}

#[repr(C)]
pub struct TaskInfoFs {
    pub cwd: *mut u8,
    pub utilizedBy: u32,
    pub LOCK_FS: *mut c_void,
}

#[repr(C)]
pub struct TaskInfoFiles {
    pub fdBitmap: *mut u8,
    pub utilizedBy: u32,
    pub WLOCK_FILES: SpinlockCnt,
}

#[repr(C)]
pub struct TaskInfoSignal {
    pub utilizedBy: u32,
    pub LOCK_SIGNAL: *mut c_void,
}

#[repr(C)]
pub struct LinkedList {
    pub firstObject: *mut c_void,
}

//
// Globals
//

extern "C" {
    static mut firstTask: *mut Task;
    static mut currentTask: *mut Task;
    static mut dummyTask: *mut Task;

    static mut tasksInitiated: bool;

    static entryCmdline: [u8; 0];
    static dummyCmdline: [u8; 0];
}

//
// Task list management
//

pub unsafe fn task_list_allocate() -> *mut Task {
    spinlockCntWriteAcquire(&mut TASK_LL_MODIFY);
    let target = malloc(core::mem::size_of::<Task>()) as *mut Task;
    memset(target as *mut c_void, 0, core::mem::size_of::<Task>());

    core::arch::asm!("cli");

    let mut browse = firstTask;
    while !(*browse).next.is_null() {
        browse = (*browse).next;
    }

    (*browse).next = target;

    core::arch::asm!("sti");
    spinlockCntWriteRelease(&mut TASK_LL_MODIFY);
    target
}

pub unsafe fn task_list_destroy(target: *mut Task) {
    spinlockCntWriteAcquire(&mut TASK_LL_MODIFY);
    core::arch::asm!("cli");

    let mut prev = firstTask;
    while !(*prev).next.is_null() {
        if (*prev).next == target {
            break;
        }
        prev = (*prev).next;
    }

    (*prev).next = (*target).next;

    core::arch::asm!("sti");
    spinlockCntWriteRelease(&mut TASK_LL_MODIFY);

    free(target as *mut c_void);
}

//
// Task creation
//

pub unsafe fn task_create(
    id: u32,
    rip: u64,
    kernel_task: bool,
    pagedir: *mut c_void,
) -> *mut Task {
    let task = task_list_allocate();

    let cs = if kernel_task {
        GDT_KERNEL_CODE
    } else {
        GDT_USER_CODE | DPL_USER
    };

    let ds = if kernel_task {
        GDT_KERNEL_DATA
    } else {
        GDT_USER_DATA | DPL_USER
    };

    (*task).registers.cs = cs;
    (*task).registers.ds = ds;
    (*task).registers.usermode_ss = ds;
    (*task).registers.rflags = 0x200;
    (*task).registers.rip = rip;
    (*task).registers.usermode_rsp = 0x0000_7fff_ffff_f000;

    (*task).id = id as u64;
    (*task).tgid = id as u64;
    (*task).sid = 1;
    (*task).kernel_task = kernel_task;
    (*task).state = TASK_STATE_CREATED;

    (*task).infoPd = taskInfoPdAllocate(false);
    (*(*task).infoPd).pagedir = pagedir;

    let tss = VirtualAllocate(USER_STACK_PAGES) as usize;
    memset(tss as *mut c_void, 0, USER_STACK_PAGES * BLOCK_SIZE);
    (*task).whileTssRsp = tss as u64 + (USER_STACK_PAGES * BLOCK_SIZE) as u64;

    (*task).parent = firstTask;

    task
}

//
// Task kill
//

pub unsafe fn task_kill(id: u32, _ret: u16) {
    let mut browse = firstTask;
    while !browse.is_null() {
        if (*browse).id == id as u64 {
            break;
        }
        browse = (*browse).next;
    }

    if browse.is_null() {
        return;
    }

    taskInfoFilesDiscard((*browse).infoFiles, browse);
    taskInfoPdDiscard((*browse).infoPd);

    (*browse).state = TASK_STATE_DEAD;

    if browse == currentTask {
        core::arch::asm!("sti");
        loop {}
    }
}

//
// Init
//

pub unsafe fn initiate_tasks() {
    firstTask = malloc(core::mem::size_of::<Task>()) as *mut Task;
    memset(firstTask as *mut c_void, 0, core::mem::size_of::<Task>());

    currentTask = firstTask;
    (*currentTask).id = KERNEL_TASK_ID;
    (*currentTask).state = TASK_STATE_READY;
    (*currentTask).kernel_task = true;

    (*currentTask).infoPd = taskInfoPdAllocate(false);
    (*(*currentTask).infoPd).pagedir = GetPageDirectory();

    (*currentTask).infoFs = taskInfoFsAllocate();
    (*currentTask).infoFiles = taskInfoFilesAllocate();

    let tss = VirtualAllocate(USER_STACK_PAGES) as usize;
    memset(tss as *mut c_void, 0, USER_STACK_PAGES * BLOCK_SIZE);
    (*currentTask).whileTssRsp = tss as u64 + (USER_STACK_PAGES * BLOCK_SIZE) as u64;

    debugf(b"[tasks] Current execution ready for multitasking\n\0".as_ptr());
    tasksInitiated = true;

    dummyTask = task_create(1, kernelDummyEntry as u64, true, GetPageDirectory());
    (*dummyTask).state = TASK_STATE_DUMMY;
}

//
// Dummy kernel task
//

#[no_mangle]
pub extern "C" fn kernelDummyEntry() -> ! {
    loop {
        unsafe { core::arch::asm!("pause") }
    }
}
