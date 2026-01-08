use std::cmp;

const FAT_ATTRIB_LFN: u8 = 0x0F;
const FAT_ATTRIB_DIRECTORY: u8 = 0x10;
const FAT_ATTRIB_ARCHIVE: u8 = 0x20;
const LFN_ORDER_FINAL: u8 = 0x40;
const LFN_MAX: usize = 20;
const LFN_MAX_TOTAL_CHARS: usize = 260;

const SECONDS_PER_MINUTE: u32 = 60;
const SECONDS_PER_HOUR: u32 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: u32 = 24 * SECONDS_PER_HOUR;

/// Number of seconds from 1970 to 1980
const SECONDS_FROM_1970_TO_1980: u32 = 315532800;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FAT32BootSector {
    sectors_per_cluster: u32,
    extended_section: FAT32ExtendedSection,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FAT32ExtendedSection {
    root_cluster: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FAT32 {
    bootsec: FAT32BootSector,
    offset_clusters: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FAT32DirectoryEntry {
    name: [u8; 8],
    ext: [u8; 3],
    attrib: u8,
    clusterhigh: u16,
    clusterlow: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FAT32LFN {
    order: u8,
    first_five: [u8; 10],
    type_byte: u8,
    checksum: u8,
    next_six: [u8; 12],
    zero: u16,
    last_two: [u8; 4],
}

#[derive(Clone, Copy, Default)]
struct FAT32TraverseResult {
    directory: u32,
    index: usize,
    dir_entry: FAT32DirectoryEntry,
}

/// Convert cluster number to LBA
fn fat32_cluster_to_lba(fat: &FAT32, cluster: u32) -> usize {
    fat.offset_clusters + ((cluster - 2) as usize) * fat.bootsec.sectors_per_cluster as usize
}

/// Check if short filename is possible
fn fat32_is_short_filename_possible(filename: &[u8]) -> i32 {
    if filename.len() > 12 {
        return -1;
    }

    let mut dotat = 0;
    for (i, &c) in filename.iter().enumerate() {
        if c == b'.' {
            dotat = i;
        } else if !((c >= b'A' && c <= b'Z') || (c >= b'0' && c <= b'9') || c == b' ') {
            return -1;
        }
    }

    if dotat > 0 && dotat < filename.len().saturating_sub(4) {
        return -1;
    }

    dotat as i32
}

/// Copy LFN entry into buffer
fn fat32_lfn_memcpy(lfn_name: &mut [u8], lfn: &FAT32LFN, index: usize) {
    let target = &mut lfn_name[index * 13..index * 13 + 13];
    target[0] = lfn.first_five[0];
    target[1] = lfn.first_five[2];
    target[2] = lfn.first_five[4];
    target[3] = lfn.first_five[6];
    target[4] = lfn.first_five[8];

    target[5] = lfn.next_six[0];
    target[6] = lfn.next_six[2];
    target[7] = lfn.next_six[4];
    target[8] = lfn.next_six[6];
    target[9] = lfn.next_six[8];
    target[10] = lfn.next_six[10];

    target[11] = lfn.last_two[0];
    target[12] = lfn.last_two[2];
}

/// Convert short filename to normal string
fn fat32_sfn_to_normal(target: &mut [u8], dirent: &FAT32DirectoryEntry) -> usize {
    let mut i = 0;
    while i < 8 && dirent.name[i] != b' ' {
        target[i] = dirent.name[i];
        i += 1;
    }

    if dirent.ext[0] != b' ' && dirent.ext[0] != 0 {
        target[i] = b'.';
        i += 1;
        for j in 0..3 {
            if dirent.ext[j] == b' ' {
                break;
            }
            target[i] = dirent.ext[j];
            i += 1;
        }
    }

    if i < target.len() {
        target[i] = 0;
    }

    i + 1
}

/// Leap year check
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Days in month
fn days_in_month(year: i32, month: i32) -> i32 {
    let days_per_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if month == 2 && is_leap_year(year) {
        29
    } else {
        days_per_month[(month - 1) as usize]
    }
}

/// Days since 1980-01-01
fn days_since_1980(year: i32, month: i32, day: i32) -> i32 {
    let mut days = 0;
    for y in 1980..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += days_in_month(year, m);
    }
    days += day - 1;
    days
}

/// Convert FAT date/time to Unix timestamp
fn fat32_unix_time(fat_date: u16, fat_time: u16) -> u32 {
    let year = ((fat_date >> 9) & 0x7F) as i32 + 1980;
    let month = ((fat_date >> 5) & 0x0F) as i32;
    let day = (fat_date & 0x1F) as i32;

    let hour = ((fat_time >> 11) & 0x1F) as i32;
    let minute = ((fat_time >> 5) & 0x3F) as i32;
    let second = ((fat_time & 0x1F) * 2) as i32;

    let days = days_since_1980(year, month, day) + (365 * 10 + 2); // 1970 -> 1980
    (days * SECONDS_PER_DAY as i32 + hour * SECONDS_PER_HOUR as i32
        + minute * SECONDS_PER_MINUTE as i32 + second) as u32
}

/// Combine high/low 16-bit cluster
fn fat_comb_high_low(high: u16, low: u16) -> u32 {
    ((high as u32) << 16) | low as u32
}
