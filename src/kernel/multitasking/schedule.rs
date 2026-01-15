#![no_std]

use core::ffi::c_void;
use core::ptr;

//
// Constants
//

const SCHEDULE_DEBUG: bool = false;
const GDT_KERNEL_CODE: u64 = 0x08;

const TASK_STATE_READY: i32 = 0;
const TASK_STATE_SIGKILLED: i32 = 5;

const EXTRAS_INVOLUTARY_WAKEUP: u32 = 1 << 3;

const SIGALRM: usize = 14;

const MSRID_FSBASE: u32 = 0xC0000100;
const MSRID_GSBASE: u32 = 0xC0000101;
const MSRID_KERNEL_GSBASE: u32 = 0xC0000102;

//
// Externals
//

extern "C" {
    static mut tasksInitiated: bool;

    static mut currentTask: *mut Task;
    static mut firstTask: *mut Task;
    static mut dummyTask: *mut Task;

    static mut timerTicks: u64;

    static mut tssPtr: *mut TSSPtr;
    static mut threadInfo: ThreadInfo;

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn spinlockRelease(lock: *mut c_void);

    fn signalsRevivableState(state: i32) -> bool;
    fn signalsPendingQuick(task: *mut Task) -> bool;
    fn signalsPendingHandleSched(task: *mut Task);

    fn atomicRead64(ptr: *const u64) -> u64;
    fn atomicWrite64(ptr: *mut u64, val: u64);
    fn atomicBitmapSet(bitmap: *mut u64, bit: usize);

    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);

    fn ChangePageDirectoryFake(pagedir: *const u64);
    fn VirtualToPhysical(addr: usize) -> usize;

    fn asm_finalize(rsp: usize, pagedir_phys: usize);
}

//
// Structures
//

#[repr(C)]
pub struct AsmPassedInterrupt {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub usermode_rsp: u64,
    pub ss: u64,
}

#[repr(C)]
pub struct TSSPtr {
    pub rsp0: u64,
}

#[repr(C)]
pub struct ITimer {
    pub at: u64,
    pub reset: u64,
}

#[repr(C)]
pub struct SignalInfo {
    pub itimerReal: ITimer,
}

#[repr(C)]
pub struct PageInfo {
    pub pagedir: *const u64,
}

#[repr(C)]
pub struct Task {
    pub id: u64,
    pub state: i32,
    pub extras: u32,
    pub kernel_task: bool,

    pub next: *mut Task,
    pub spinlockQueueEntry: *mut c_void,

    pub forcefulWakeupTimeUnsafe: u64,

    pub registers: AsmPassedInterrupt,

    pub whileTssRsp: u64,
    pub whileSyscallRsp: u64,

    pub fsbase: u64,
    pub gsbase: u64,

    pub fpuenv: [u8; 512],
    pub mxcsr: u32,

    pub pagedirOverride: *const u64,
    pub infoPd: *const PageInfo,

    pub infoSignals: *mut SignalInfo,
    pub sigPendingList: u64,
}

#[repr(C)]
pub struct ThreadInfo {
    pub syscall_stack: u64,
}

//
// MSR helpers
//

#[inline(always)]
unsafe fn wrmsr(msr: u32, value: u64) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u32,
        options(nostack, preserves_flags),
    );
}

//
// Scheduler
//

#[no_mangle]
pub unsafe extern "C" fn schedule(rsp: u64) {
    if !tasksInitiated {
        return;
    }

    let cpu = rsp as *mut AsmPassedInterrupt;

    let mut next = (*currentTask).next;
    if next.is_null() {
        next = firstTask;
    }

    let mut full_run = 0;

    while (*next).state != TASK_STATE_READY {
        if signalsRevivableState((*next).state) && signalsPendingQuick(next) {
            (*next).extras |= EXTRAS_INVOLUTARY_WAKEUP;
            (*next).forcefulWakeupTimeUnsafe = 0;
            (*next).state = TASK_STATE_READY;
            break;
        }

        if (*next).forcefulWakeupTimeUnsafe != 0
            && (*next).forcefulWakeupTimeUnsafe <= timerTicks
        {
            (*next).state = TASK_STATE_READY;
            (*next).extras |= EXTRAS_INVOLUTARY_WAKEUP;
            (*next).forcefulWakeupTimeUnsafe = 0;
            break;
        }

        next = (*next).next;
        if next.is_null() {
            full_run += 1;
            if full_run > 2 {
                break;
            }
            next = firstTask;
        }
    }

    if next.is_null() {
        next = dummyTask;
    }

    let old = currentTask;
    currentTask = next;

    if (*old).state != TASK_STATE_READY && !(*old).spinlockQueueEntry.is_null() {
        spinlockRelease((*old).spinlockQueueEntry);
        (*old).spinlockQueueEntry = ptr::null_mut();
    }

    if !(*next).kernel_task {
        let rt_at = atomicRead64(&(*(*next).infoSignals).itimerReal.at);
        let rt_reset = atomicRead64(&(*(*next).infoSignals).itimerReal.reset);

        if rt_at != 0 && rt_at <= timerTicks {
            atomicBitmapSet(&mut (*next).sigPendingList, SIGALRM);

            if rt_reset == 0 {
                atomicWrite64(&mut (*(*next).infoSignals).itimerReal.at, 0);
            } else {
                atomicWrite64(
                    &mut (*(*next).infoSignals).itimerReal.at,
                    timerTicks + rt_reset,
                );
            }
        }
    }

    if !(*next).kernel_task && ((*next).registers.cs & GDT_KERNEL_CODE) == 0 {
        signalsPendingHandleSched(next);
        if (*next).state == TASK_STATE_SIGKILLED {
            currentTask = old;
            schedule(rsp);
            return;
        }
    }

    (*tssPtr).rsp0 = (*next).whileTssRsp;
    threadInfo.syscall_stack = (*next).whileSyscallRsp;

    wrmsr(MSRID_FSBASE, (*next).fsbase);
    wrmsr(MSRID_GSBASE, (*next).gsbase);
    wrmsr(MSRID_KERNEL_GSBASE, &threadInfo as *const _ as u64);

    memcpy(
        &mut (*old).registers as *mut _ as *mut c_void,
        cpu as *const c_void,
        core::mem::size_of::<AsmPassedInterrupt>(),
    );

    if !(*old).kernel_task {
        core::arch::asm!("fxsave [{}]", in(reg) &(*old).fpuenv);
        core::arch::asm!("stmxcsr [{}]", in(reg) &(*old).mxcsr);
    }

    if !(*next).kernel_task {
        core::arch::asm!("fxrstor [{}]", in(reg) &(*next).fpuenv);
        core::arch::asm!("ldmxcsr [{}]", in(reg) &(*next).mxcsr);
    }

    let iret_rsp =
        ((*next).whileTssRsp - core::mem::size_of::<AsmPassedInterrupt>())
            as *mut AsmPassedInterrupt;

    memcpy(
        iret_rsp as *mut c_void,
        &(*next).registers as *const _ as *const c_void,
        core::mem::size_of::<AsmPassedInterrupt>(),
    );

    let pagedir = if !(*next).pag*
