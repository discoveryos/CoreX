#![no_std]
#![no_main]

use core::mem::{size_of, zeroed};
use core::ptr::copy_nonoverlapping;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::bootloader::*;
use crate::gdt::*;
use crate::linked_list::*;
use crate::paging::*;
use crate::syscalls::*;
use crate::task::*;
use crate::timer::*;
use crate::util::*;

pub const NSIG: usize = 64;

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum SignalInternal {
    Core,
    Term,
    Ign,
    Stop,
    Cont,
}

static mut SIGNAL_INTERNAL_DECISIONS: [SignalInternal; NSIG] = [SignalInternal::Core; NSIG];

pub fn initiate_signal_defs() {
    unsafe {
        SIGNAL_INTERNAL_DECISIONS[SIGABRT as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGALRM as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGBUS as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGCHLD as usize] = SignalInternal::Ign;
        SIGNAL_INTERNAL_DECISIONS[SIGCONT as usize] = SignalInternal::Cont;
        SIGNAL_INTERNAL_DECISIONS[SIGFPE as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGHUP as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGILL as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGINT as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGIO as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGIOT as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGKILL as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGPIPE as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGPOLL as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGPROF as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGPWR as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGQUIT as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGSEGV as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGSTKFLT as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGSTOP as usize] = SignalInternal::Stop;
        SIGNAL_INTERNAL_DECISIONS[SIGTSTP as usize] = SignalInternal::Stop;
        SIGNAL_INTERNAL_DECISIONS[SIGSYS as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGTERM as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGTRAP as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGTTIN as usize] = SignalInternal::Stop;
        SIGNAL_INTERNAL_DECISIONS[SIGTTOU as usize] = SignalInternal::Stop;
        SIGNAL_INTERNAL_DECISIONS[SIGUNUSED as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGURG as usize] = SignalInternal::Ign;
        SIGNAL_INTERNAL_DECISIONS[SIGUSR1 as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGUSR2 as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGVTALRM as usize] = SignalInternal::Term;
        SIGNAL_INTERNAL_DECISIONS[SIGXCPU as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGXFSZ as usize] = SignalInternal::Core;
        SIGNAL_INTERNAL_DECISIONS[SIGWINCH as usize] = SignalInternal::Ign;
    }
}

/// Representation of CPU state during interrupt/syscall
#[repr(C)]
#[derive(Copy, Clone)]
pub struct AsmPassedInterrupt {
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rsp: u64,
    pub rip: u64,
    pub rflags: u64,
    pub cs: u64,
    pub ds: u64,
    pub usermode_rsp: u64,
    pub usermode_ss: u64,
    pub error: u64,
    pub interrupt: u64,
}

/// Representation of signal context (for user-space handler)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct SigContext {
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rsp: u64,
    pub rip: u64,
    pub eflags: u64,
    pub cs: u64,
    pub ss: u64,
    pub err: u64,
    pub trapno: u64,
    pub oldmask: u64,
    pub cr2: u64,
    pub fpstate: *mut FpState,
    pub reserved1: [u64; 8],
}

/// FPU state
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FpState {
    pub fxsave_area: [u8; 512],
    pub mxcsr: u32,
    pub _padding: [u8; 12],
}

/// Convert `AsmPassedInterrupt` → `SigContext`
pub unsafe fn asm_to_ucontext(passed: &AsmPassedInterrupt, ucontext: &mut SigContext) {
    ucontext.r8 = passed.r8;
    ucontext.r9 = passed.r9;
    ucontext.r10 = passed.r10;
    ucontext.r11 = passed.r11;
    ucontext.r12 = passed.r12;
    ucontext.r13 = passed.r13;
    ucontext.r14 = passed.r14;
    ucontext.r15 = passed.r15;
    ucontext.rdi = passed.rdi;
    ucontext.rsi = passed.rsi;
    ucontext.rbp = passed.rbp;
    ucontext.rbx = passed.rbx;
    ucontext.rdx = passed.rdx;
    ucontext.rax = passed.rax;
    ucontext.rcx = passed.rcx;
    ucontext.rsp = passed.usermode_rsp;
    ucontext.rip = passed.rip;
    ucontext.eflags = passed.rflags;
    ucontext.cs = passed.cs;
    ucontext.ss = passed.usermode_ss;
    ucontext.err = 0;
    ucontext.trapno = 0;
    ucontext.oldmask = 0;
    ucontext.cr2 = 0;
    ucontext.fpstate = core::ptr::null_mut();
    ucontext.reserved1 = [0; 8];
}

/// Convert `SigContext` → `AsmPassedInterrupt`
pub unsafe fn ucontext_to_asm(ucontext: &SigContext, passed: &mut AsmPassedInterrupt) {
    passed.r8 = ucontext.r8;
    passed.r9 = ucontext.r9;
    passed.r10 = ucontext.r10;
    passed.r11 = ucontext.r11;
    passed.r12 = ucontext.r12;
    passed.r13 = ucontext.r13;
    passed.r14 = ucontext.r14;
    passed.r15 = ucontext.r15;
    passed.rdi = ucontext.rdi;
    passed.rsi = ucontext.rsi;
    passed.rbp = ucontext.rbp;
    passed.rbx = ucontext.rbx;
    passed.rdx = ucontext.rdx;
    passed.rax = ucontext.rax;
    passed.rcx = ucontext.rcx;
    passed.usermode_rsp = ucontext.rsp;
    passed.rip = ucontext.rip;
    passed.rflags = ucontext.eflags;
    passed.cs = ucontext.cs;
    passed.usermode_ss = ucontext.ss;
    passed.error = ucontext.err;
    passed.interrupt = ucontext.trapno;
}

/// Check quickly if a task has pending signals
pub fn signals_pending_quick(task: &Task) -> bool {
    if task.kernel_task { return false; }
    let pending_list = task.sig_pending_list.load(Ordering::SeqCst);
    let unblocked_list = pending_list & !task.sig_block_list;
    for i in 0..NSIG {
        if (unblocked_list & (1 << i)) == 0 { continue; }
        let action = &task.info_signals.signals[i];
        let user_handler = unsafe { core::mem::transmute::<usize, SignalHandler>(action.sa_handler.load(Ordering::SeqCst)) };
        if user_handler == SignalHandler::Ignore { continue; }
        unsafe {
            if user_handler == SignalHandler::Default && SIGNAL_INTERNAL_DECISIONS[i] == SignalInternal::Ign {
                continue;
            }
        }
        return true;
    }
    false
}
