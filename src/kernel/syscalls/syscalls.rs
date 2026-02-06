#![no_std]
#![no_main]

use core::fmt::Write;
use core::ptr::null_mut;

use crate::console::debugf;
use crate::fb::*;
use crate::gdt::*;
use crate::isr::*;
use crate::kb::*;
use crate::poll::*;
use crate::schedule::*;
use crate::serial::*;
use crate::string::*;
use crate::syscalls::*;
use crate::system::*;
use crate::task::*;
use crate::timer::*;
use crate::unix_socket::*;
use crate::util::*;

use crate::linked_list::*;
use crate::malloc::*;

// Configuration flags (simulate C #define)
const DEBUG_SYSCALLS_STRACE: bool = true;
const DEBUG_SYSCALLS_FAILS: bool = true;
const DEBUG_SYSCALLS_EXTRA: bool = true;
const DEBUG_SYSCALLS_STUB: bool = true;

const MAX_SYSCALLS: usize = 512;

static mut SYSCALLS: [Option<usize>; MAX_SYSCALLS] = [None; MAX_SYSCALLS];
static mut SYSCALL_COUNT: u32 = 0;

// Debug string fallback
const DEFAULT_ERR_STR: &str = "unknown";

// Typedef for syscall handlers
type SyscallHandler = extern "C" fn(u64, u64, u64, u64, u64, u64) -> u64;

// Register a syscall handler
pub unsafe fn register_syscall(id: u32, handler: *const ()) {
    if id as usize >= MAX_SYSCALLS {
        debugf("[syscalls] FATAL! Exceeded limit! limit{%d} id{%d}\n", MAX_SYSCALLS, id);
        panic!();
    }

    if SYSCALLS[id as usize].is_some() {
        debugf("[syscalls] FATAL! Duplicate syscall id{%d}\n", id);
        panic!();
    }

    SYSCALLS[id as usize] = Some(handler as usize);
    SYSCALL_COUNT += 1;
}

// Syscall dispatch
pub unsafe fn syscall_handler(regs: &mut AsmPassedInterrupt) {
    wrmsr(MSRID_KERNEL_GSBASE, &thread_info as *const _ as usize);

    let rsp_ptr = (regs as *mut _ as usize + core::mem::size_of::<AsmPassedInterrupt>()) as *mut u64;
    let rsp = *rsp_ptr;

    current_task().system_call_in_progress = true;
    current_task().syscall_regs = regs as *mut _;
    current_task().syscall_rsp = rsp;

    asm!("sti"); // enable interrupts while processing

    let id = regs.rax;
    if id as usize >= MAX_SYSCALLS {
        regs.rax = u64::MAX; // -1
        if DEBUG_SYSCALLS_FAILS {
            debugf("[syscalls] FAIL! Tried to access syscall{%d} (out of bounds)!\n", id);
        }
        goto_cleanup(rsp_ptr, regs);
        return;
    }

    let handler = SYSCALLS[id as usize].unwrap_or(0);

    if DEBUG_SYSCALLS_STRACE {
        // placeholder for strace logging
    }

    if handler == 0 {
        regs.rax = ERR(ENOSYS);
        match id {
            54 | 222..=226 | 324 | 28 => regs.rax = 0,
            _ => {}
        }
        goto_cleanup(rsp_ptr, regs);
        return;
    }

    let time_start = timer_ticks();
    let ret = (core::mem::transmute::<usize, SyscallHandler>(handler))(
        regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9,
    );
    let time_took = timer_ticks() - time_start;

    if DEBUG_SYSCALLS_STRACE {
        // placeholder for error or success logging
    }

    if RET_IS_ERR(ret) && ret == ERR(EINTR) {
        let sys_intr = calloc::<TaskSysInterrupted>(1);
        (*sys_intr).number = regs.rax;
        LinkedListPushFrontUnsafe(&mut current_task().ds_sys_intr, sys_intr as *mut _);
    }

    regs.rax = ret;

    goto_cleanup(rsp_ptr, regs);
}

// Cleanup function
unsafe fn goto_cleanup(rsp_ptr: *mut u64, regs: &mut AsmPassedInterrupt) {
    assert!(!current_task().pagedir_override);
    current_task().syscall_rsp = 0;
    current_task().syscall_regs = core::ptr::null_mut();
    current_task().system_call_in_progress = false;

    // Handle pending signals
    signals_pending_handle_sys(current_task(), rsp_ptr, regs);
}

// Debug helpers
pub unsafe fn dbg_sys_failf(args: core::fmt::Arguments) {
    if DEBUG_SYSCALLS_FAILS {
        debugf(" //{}", args);
    }
}

pub unsafe fn dbg_sys_extraf(args: core::fmt::Arguments) {
    if DEBUG_SYSCALLS_EXTRA {
        debugf(" //{}", args);
    }
}

pub unsafe fn dbg_sys_stubf(args: core::fmt::Arguments) {
    if DEBUG_SYSCALLS_STUB {
        debugf(" //{}", args);
    }
}

// Initialize all syscalls
pub unsafe fn initiate_syscalls() {
    syscall_reg_fs();
    syscalls_reg_poll();
    syscall_reg_mem();
    syscall_reg_sig();
    syscalls_reg_env();
    syscalls_reg_proc();
    syscalls_reg_clock();
    syscalls_reg_net();

    initiate_signal_defs();

    LinkedListInit(&mut DS_UNIX_SOCKET, core::mem::size_of::<UnixSocket>());
    LinkedListInit(&mut DS_POLL_ROOT, core::mem::size_of::<PollInstance>());
    LinkedListInit(&mut DS_EPOLL, core::mem::size_of::<Epoll>());

    debugf(
        "[syscalls] System calls are ready to fire: {}/{}\n",
        SYSCALL_COUNT,
        MAX_SYSCALLS,
    );
}
