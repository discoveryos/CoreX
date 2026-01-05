#![no_std]

use core::arch::asm;

// ======================================================
// Constants / globals
// ======================================================

// Fallback year if no ACPI century register exists
const CURRENT_YEAR: i32 = 2025;

const CMOS_ADDRESS: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

// Set by ACPI code if available
#[no_mangle]
pub static mut century_register: u8 = 0x00;

// ======================================================
// Externals
// ======================================================

extern "C" {
    fn outportb(port: u16, val: u8);
    fn inportb(port: u16) -> u8;
}

// ======================================================
// RTC structure
// ======================================================

#[repr(C)]
pub struct RTC {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: i32,
}

// ======================================================
// CMOS helpers
// ======================================================

unsafe fn get_update_in_progress_flag() -> bool {
    outportb(CMOS_ADDRESS, 0x0A);
    (inportb(CMOS_DATA) & 0x80) != 0
}

unsafe fn get_rtc_register(reg: u8) -> u8 {
    outportb(CMOS_ADDRESS, reg);
    inportb(CMOS_DATA)
}

// ======================================================
// RTC read
// ======================================================

pub unsafe fn read_from_cmos(rtc: &mut RTC) -> bool {
    let mut century: u8 = 0;

    let mut last_second;
    let mut last_minute;
    let mut last_hour;
    let mut last_day;
    let mut last_month;
    let mut last_year;
    let mut last_century;

    // Initial read
    while get_update_in_progress_flag() {}

    rtc.second = get_rtc_register(0x00);
    rtc.minute = get_rtc_register(0x02);
    rtc.hour = get_rtc_register(0x04);
    rtc.day = get_rtc_register(0x07);
    rtc.month = get_rtc_register(0x08);
    rtc.year = get_rtc_register(0x09) as i32;

    if century_register != 0 {
        century = get_rtc_register(century_register);
    }

    // Repeat until stable
    loop {
        last_second = rtc.second;
        last_minute = rtc.minute;
        last_hour = rtc.hour;
        last_day = rtc.day;
        last_month = rtc.month;
        last_year = rtc.year as u8;
        last_century = century;

        while get_update_in_progress_flag() {}

        rtc.second = get_rtc_register(0x00);
        rtc.minute = get_rtc_register(0x02);
        rtc.hour = get_rtc_register(0x04);
        rtc.day = get_rtc_register(0x07);
        rtc.month = get_rtc_register(0x08);
        rtc.year = get_rtc_register(0x09) as i32;

        if century_register != 0 {
            century = get_rtc_register(century_register);
        }

        if last_second == rtc.second
            && last_minute == rtc.minute
            && last_hour == rtc.hour
            && last_day == rtc.day
            && last_month == rtc.month
            && last_year == rtc.year as u8
            && last_century == century
        {
            break;
        }
    }

    let register_b = get_rtc_register(0x0B);

    // BCD → binary
    if (register_b & 0x04) == 0 {
        rtc.second = (rtc.second & 0x0F) + ((rtc.second / 16) * 10);
        rtc.minute = (rtc.minute & 0x0F) + ((rtc.minute / 16) * 10);
        rtc.hour =
            ((rtc.hour & 0x0F) + (((rtc.hour & 0x70) / 16) * 10)) | (rtc.hour & 0x80);
        rtc.day = (rtc.day & 0x0F) + ((rtc.day / 16) * 10);
        rtc.month = (rtc.month & 0x0F) + ((rtc.month / 16) * 10);
        rtc.year = (rtc.year & 0x0F) + ((rtc.year / 16) * 10);

        if century_register != 0 {
            century = (century & 0x0F) + ((century / 16) * 10);
        }
    }

    // 12h → 24h
    if (register_b & 0x02) == 0 && (rtc.hour & 0x80) != 0 {
        rtc.hour = ((rtc.hour & 0x7F) + 12) % 24;
    }

    // Full year
    if century_register != 0 {
        rtc.year += (century as i32) * 100;
    } else {
        rtc.year += (CURRENT_YEAR / 100) * 100;
        if rtc.year < CURRENT_YEAR {
            rtc.year += 100;
        }
    }

    true
}

// ======================================================
// Unix time conversion
// ======================================================

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

static DAYS_IN_MONTH: [[i32; 12]; 2] = [
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
];

pub fn rtc_to_unix(rtc: &RTC) -> u64 {
    let mut seconds: u64 = 0;

    let year = rtc.year;
    let month = rtc.month as i32 - 1;
    let day = rtc.day as i32 - 1;

    for y in 1970..year {
        seconds += (365 + is_leap_year(y) as i32) as u64 * 86400;
    }

    let leap = is_leap_year(year) as usize;
    for m in 0..month {
        seconds += DAYS_IN_MONTH[leap][m as usize] as u64 * 86400;
    }

    seconds += day as u64 * 86400;
    seconds += rtc.hour as u64 * 3600;
    seconds += rtc.minute as u64 * 60;
    seconds += rtc.second as u64;

    seconds
}
