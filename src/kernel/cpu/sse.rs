
#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};

/// ---------------------------
/// PORT I/O
/// ---------------------------
unsafe fn outportb(port: u16, value: u8) {
    core::arch::asm!("outb %al, %dx", in("dx") port, in("al") value);
}

unsafe fn inportb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("inb %dx, %al", out("al") val, in("dx") port);
    val
}

/// ---------------------------
/// RTC STRUCTS & HELPERS
/// ---------------------------
#[derive(Default)]
pub struct RTC {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u32,
}

const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;
const CURRENT_YEAR: u32 = 2025;

unsafe fn get_update_in_progress_flag() -> bool {
    outportb(CMOS_ADDR, 0x0A);
    inportb(CMOS_DATA) & 0x80 != 0
}

unsafe fn get_rtc_register(reg: u8) -> u8 {
    outportb(CMOS_ADDR, reg);
    inportb(CMOS_DATA)
}

pub unsafe fn read_from_cmos(rtc: &mut RTC) {
    while get_update_in_progress_flag() {}
    rtc.second = get_rtc_register(0x00);
    rtc.minute = get_rtc_register(0x02);
    rtc.hour = get_rtc_register(0x04);
    rtc.day = get_rtc_register(0x07);
    rtc.month = get_rtc_register(0x08);
    rtc.year = get_rtc_register(0x09) as u32 + (CURRENT_YEAR / 100) * 100;
}

/// Converts RTC to Unix timestamp
pub fn rtc_to_unix(rtc: &RTC) -> u64 {
    let days_in_month = [
        31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];

    let mut seconds = 0;
    for y in 1970..rtc.year {
        seconds += if is_leap_year(y) { 366 * 86400 } else { 365 * 86400 };
    }
    let leap = is_leap_year(rtc.year);
    for m in 0..(rtc.month - 1) {
        seconds += days_in_month[m as usize] * 86400;
        if leap && m == 1 {
            seconds += 86400;
        }
    }
    seconds += (rtc.day - 1) as u64 * 86400;
    seconds += rtc.hour as u64 * 3600;
    seconds += rtc.minute as u64 * 60;
    seconds += rtc.second as u64;
    seconds
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// ---------------------------
/// TIMER & APIC GLOBALS
/// ---------------------------
static mut TIMER_TICKS: u64 = 0;
static mut APIC_FREQ: u32 = 0;
static mut TIMER_BOOT_UNIX: u64 = 0;

const PIT_CHANNEL0: u16 = 0x40;
const PIT_CMD: u16 = 0x43;
const PIT_INPUT_FREQ: u32 = 1_193_182;

/// ---------------------------
/// DEBUG FUNCTION STUB
/// ---------------------------
fn debugf(fmt: &str, val: impl core::fmt::Display) {
    // Stub: replace with kernel printing
    let _ = (fmt, val);
}

/// ---------------------------
/// PIT TIMER
/// ---------------------------
pub unsafe fn initiate_pit_timer(reload_value: u32) {
    let frequency = PIT_INPUT_FREQ / reload_value;

    outportb(PIT_CMD, 0b00110100);

    let l = (frequency & 0xFF) as u8;
    let h = ((frequency >> 8) & 0xFF) as u8;

    outportb(PIT_CHANNEL0, l);
    outportb(PIT_CHANNEL0, h);

    let mut rtc = RTC::default();
    read_from_cmos(&mut rtc);

    TIMER_TICKS = 0;
    TIMER_BOOT_UNIX = rtc_to_unix(&rtc);

    debugf("[timer] Ready to fire: frequency={}", frequency);
}

/// Called on timer IRQ
pub extern "C" fn timer_tick(_rsp: u64) {
    unsafe { TIMER_TICKS += 1; }
    // schedule(_rsp); // call scheduler here
}

/// Busy-wait sleep
pub fn sleep(time: u64) {
    let target = unsafe { TIMER_TICKS + time };
    while unsafe { TIMER_TICKS < target } {
        hand_control();
    }
}

/// ---------------------------
/// APIC TIMER (STUB FUNCTIONS)
/// ---------------------------
const APIC_REGISTER_TIMER_DIV: u32 = 0x3E0;
const APIC_REGISTER_TIMER_INITCNT: u32 = 0x380;
const APIC_REGISTER_TIMER_CURRCNT: u32 = 0x390;
const APIC_REGISTER_LVT_TIMER: u32 = 0x320;
const APIC_LVT_TIMER_MODE_PERIODIC: u32 = 0x20000;

unsafe fn apic_write(_reg: u32, _val: u32) {
    // write to LAPIC MMIO
}

unsafe fn apic_read(_reg: u32) -> u32 {
    0xFFFF_FFFF
}

unsafe fn ioapic_redirect(_irq: u8, _mask: bool) -> u8 {
    0
}

unsafe fn register_irq_handler(_irq: u8, _handler: extern "C" fn(u64)) {}

unsafe fn irq_per_core_allocate(_core: u8, _lapic_id: &mut u32) -> u8 {
    32
}

fn hand_control() {
    // Kernel yield / task switching stub
}

/// ---------------------------
/// INITIATE APIC TIMER
/// ---------------------------
pub unsafe fn initiate_apic_timer() {
    let wait_ms: u64 = 10;

    initiate_pit_timer(1000);

    let ioapic_irq = ioapic_redirect(0, false);
    register_irq_handler(ioapic_irq, timer_tick);

    apic_write(APIC_REGISTER_TIMER_DIV, 0x3);
    apic_write(APIC_REGISTER_TIMER_INITCNT, 0xFFFF_FFFF);

    let target = TIMER_TICKS + wait_ms;
    while TIMER_TICKS < target {}

    apic_write(APIC_REGISTER_LVT_TIMER, 0x10000);
    let ticks_in_xms = 0xFFFF_FFFF - apic_read(APIC_REGISTER_TIMER_CURRCNT);

    let mut lapic_id = 0;
    let targ_irq = irq_per_core_allocate(0, &mut lapic_id);

    APIC_FREQ = (ticks_in_xms / wait_ms) as u32;

    apic_write(
        APIC_REGISTER_LVT_TIMER,
        targ_irq as u32 | APIC_LVT_TIMER_MODE_PERIODIC,
    );
    apic_write(APIC_REGISTER_TIMER_DIV, 0x3);
    apic_write(APIC_REGISTER_TIMER_INITCNT, APIC_FREQ);

    ioapic_redirect(0, true);
    register_irq_handler(targ_irq, timer_tick);
}
