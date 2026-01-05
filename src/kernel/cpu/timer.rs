#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};

use crate::apic::*;
use crate::idt::*;
use crate::rtc::*;
use crate::schedule::*;
use crate::system::*;

// Timer globals
static mut TIMER_TICKS: u64 = 0;
static mut APIC_FREQ: u32 = 0;
static mut TIMER_BOOT_UNIX: u64 = 0;

const PIT_CHANNEL0: u16 = 0x40;
const PIT_CMD: u16 = 0x43;
const PIT_INPUT_FREQ: u32 = 1_193_182; // PIT clock

/// Send byte to port
unsafe fn outportb(port: u16, value: u8) {
    core::arch::asm!("outb %al, %dx", in("dx") port, in("al") value);
}

/// Read byte from port
unsafe fn inportb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("inb %dx, %al", out("al") val, in("dx") port);
    val
}

/// Initialize PIT timer with a reload value
pub unsafe fn initiate_pit_timer(reload_value: u32) {
    let frequency = PIT_INPUT_FREQ / reload_value;

    // Configure PIT channel 0, mode 2, lobyte/hibyte
    outportb(PIT_CMD, 0b00110100);

    let l = (frequency & 0xFF) as u8;
    let h = ((frequency >> 8) & 0xFF) as u8;

    outportb(PIT_CHANNEL0, l);
    outportb(PIT_CHANNEL0, h);

    // Read RTC for boot timestamp
    let mut rtc = RTC::default();
    read_from_cmos(&mut rtc);

    TIMER_TICKS = 0;
    TIMER_BOOT_UNIX = rtc_to_unix(&rtc);

    debugf("[timer] Ready to fire: frequency={} Hz\n", frequency);
}

/// Increment timer ticks and invoke scheduler
pub extern "C" fn timer_tick(rsp: u64) {
    unsafe { TIMER_TICKS += 1; }
    schedule(rsp);
}

/// Sleep function using busy wait
pub fn sleep(time: u64) {
    let target = unsafe { TIMER_TICKS + time };
    while unsafe { TIMER_TICKS < target } {
        hand_control();
    }
}

/// Initialize Local APIC timer and calibrate using PIT
pub unsafe fn initiate_apic_timer() {
    let wait_ms: u64 = 10;

    // Step 1: Setup PIT as reference
    initiate_pit_timer(1000);

    // Step 2: Mask IOAPIC interrupt and register PIT handler
    let ioapic_irq = ioapic_redirect(0, false);
    register_irq_handler(ioapic_irq, timer_tick);

    // Step 3: Setup Local APIC timer max count
    apic_write(APIC_REGISTER_TIMER_DIV, 0x3); // div16
    apic_write(APIC_REGISTER_TIMER_INITCNT, 0xFFFF_FFFF);

    // Wait for PIT ticks to accumulate
    let target = TIMER_TICKS + wait_ms;
    while TIMER_TICKS < target {}

    // Mask APIC timer and measure elapsed ticks
    apic_write(APIC_REGISTER_LVT_TIMER, 0x10000);
    let ticks_in_xms = 0xFFFF_FFFF - apic_read(APIC_REGISTER_TIMER_CURRCNT);

    // Allocate APIC IRQ for this core
    let mut lapic_id = 0;
    let targ_irq = irq_per_core_allocate(0, &mut lapic_id);

    APIC_FREQ = (ticks_in_xms / wait_ms) as u32;

    // Configure APIC timer periodic mode
    apic_write(
        APIC_REGISTER_LVT_TIMER,
        targ_irq as u32 | APIC_LVT_TIMER_MODE_PERIODIC,
    );
    apic_write(APIC_REGISTER_TIMER_DIV, 0x3);
    apic_write(APIC_REGISTER_TIMER_INITCNT, APIC_FREQ);

    // Mask old PIT IRQ
    ioapic_redirect(0, true);
    register_irq_handler(targ_irq, timer_tick);
}
