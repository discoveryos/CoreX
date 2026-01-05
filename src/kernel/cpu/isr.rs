#![no_std]

use core::arch::asm;
use core::ptr;

// ======================================================
// Externals (kernel-provided)
// ======================================================

extern "C" {
    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn outportb(port: u16, val: u8);
    fn apicWrite(offset: u32, value: u32);

    fn set_idt_gate(n: usize, handler: u64, flags: u8);
    fn set_idt();

    fn initiateAPIC();
    fn syscallHandler(cpu: *mut AsmPassedInterrupt);

    static asm_isr_redirect_table: [u64; 256];
    fn isr255();
    fn isr128();

    static mut dsIrqHandler: LinkedList;
    static mut currentTask: *mut Task;
    static mut tasksInitiated: bool;
    static timerTicks: u64;
}

// ======================================================
// Constants
// ======================================================

const ANSI_RED: &str = "\x1b[31m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_RESET: &str = "\x1b[0m";

const SCHED_PAGE_FAULT_MAGIC_ADDRESS: u64 = 0xDEADBEEF;
const KERNEL_TASK_ID: i32 = 0;

// ======================================================
// Data structures (must match ASM layout exactly)
// ======================================================

#[repr(C)]
pub struct AsmPassedInterrupt {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub usermode_rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
    pub cs: u64,
    pub usermode_ss: u64,
    pub error: u64,
    pub interrupt: u64,
    pub ds: u64,
}

#[repr(C)]
pub struct IrqHandler {
    pub id: u8,
    pub handler: extern "C" fn(*mut AsmPassedInterrupt),
    pub argument: *mut AsmPassedInterrupt,
    pub _ll: LinkedListNode,
}

// ======================================================
// Strings
// ======================================================

static FORMAT: &[u8] = b"[isr] Kernel panic: %s!\n\0";
static EQUALS: &[u8] = b"=======================================\0";

static EXCEPTIONS: [&[u8]; 32] = [
    b"Division By Zero\0",
    b"Debug\0",
    b"Non Maskable Interrupt\0",
    b"Breakpoint\0",
    b"Into Detected Overflow\0",
    b"Out of Bounds\0",
    b"Invalid Opcode\0",
    b"No Coprocessor\0",
    b"Double Fault\0",
    b"Coprocessor Segment Overrun\0",
    b"Bad TSS\0",
    b"Segment Not Present\0",
    b"Stack Fault\0",
    b"General Protection Fault\0",
    b"Page Fault\0",
    b"Unknown Interrupt\0",
    b"Coprocessor Fault\0",
    b"Alignment Check\0",
    b"Machine Check\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
    b"Reserved\0",
];

// ======================================================
// Utilities
// ======================================================

unsafe fn register_dump(regs: *mut AsmPassedInterrupt) {
    let r = &*regs;
    debugf(
        b"%s REGDUMP %s\n\0".as_ptr(),
        EQUALS.as_ptr(),
        EQUALS.as_ptr(),
    );
    debugf(
        b"RIP=%lx RSP=%lx RFLAGS=%lx INT=%lx ERR=%lx\n\0".as_ptr(),
        r.rip,
        r.usermode_rsp,
        r.rflags,
        r.interrupt,
        r.error,
    );
    debugf(
        b"[regdump] regs{%lx} timerTicks{%ld} task{%lx}\n\0".as_ptr(),
        regs,
        timerTicks,
        currentTask,
    );
}

// ======================================================
// PIC
// ======================================================

unsafe fn disable_pic() {
    outportb(0x21, 0xFF);
    outportb(0xA1, 0xFF);
}

unsafe fn remap_pic() {
    outportb(0x20, 0x11);
    outportb(0xA0, 0x11);
    outportb(0x21, 0x20);
    outportb(0xA1, 0x28);
    outportb(0x21, 0x04);
    outportb(0xA1, 0x02);
    outportb(0x21, 0x01);
    outportb(0xA1, 0x01);
    outportb(0x21, 0x00);
    outportb(0xA1, 0x00);
    disable_pic();
}

// ======================================================
// Initialization
// ======================================================

pub unsafe fn initiate_isr() {
    remap_pic();

    for i in 0..48 {
        set_idt_gate(i, asm_isr_redirect_table[i], 0x8E);
    }

    set_idt_gate(3, asm_isr_redirect_table[3], 0xEE);
    set_idt_gate(0xFF, isr255 as u64, 0x8E);
    set_idt_gate(0x80, isr128 as u64, 0xEE);

    set_idt();
    initiateAPIC();

    asm!("sti", options(nomem, nostack));
}

// ======================================================
// Main interrupt handler (called from ASM)
// ======================================================

#[no_mangle]
pub unsafe extern "C" fn handle_interrupt(rsp: u64) {
    let cpu = rsp as *mut AsmPassedInterrupt;
    let int_no = (*cpu).interrupt;

    // IRQs
    if (32..=47).contains(&int_no) {
        if int_no >= 40 {
            outportb(0xA0, 0x20);
        }
        outportb(0x20, 0x20);
        apicWrite(0xB0, 0);

        let mut node = dsIrqHandler.firstObject;
        while !node.is_null() {
            let h = node as *mut IrqHandler;
            if (*h).id as u64 == int_no {
                ((*h).handler)(if (*h).argument.is_null() {
                    cpu
                } else {
                    (*h).argument
                });
            }
            node = (*node)._ll.next;
        }
        return;
    }

    // Syscall
    if int_no == 0x80 {
        syscallHandler(cpu);
        return;
    }

    // Exceptions
    register_dump(cpu);
    debugf(FORMAT.as_ptr(), EXCEPTIONS[int_no as usize].as_ptr());
    panic();
}
