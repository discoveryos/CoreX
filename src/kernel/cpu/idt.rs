use core::arch::asm;
use core::mem::size_of;

// ======================================================
// Constants
// ======================================================

const IDT_ENTRIES: usize = 256;
const GDT_KERNEL_CODE: u16 = 0x28;

// ======================================================
// IDT structures
// ======================================================

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct IdtGate {
    isr_low: u16,
    kernel_cs: u16,
    ist: u8,
    attributes: u8,
    isr_mid: u16,
    isr_high: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct IdtRegister {
    limit: u16,
    base: u64,
}

// ======================================================
// Statics
// ======================================================

static mut IDT: [IdtGate; IDT_ENTRIES] = [IdtGate {
    isr_low: 0,
    kernel_cs: 0,
    ist: 0,
    attributes: 0,
    isr_mid: 0,
    isr_high: 0,
    reserved: 0,
}; IDT_ENTRIES];

static mut IDT_REG: IdtRegister = IdtRegister { limit: 0,_
