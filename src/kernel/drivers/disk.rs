#![no_std]
#![no_main]

use core::ptr::{copy_nonoverlapping, null_mut};
use core::mem::size_of;

use crate::ahci::{ahci, ahci_read, ahci_write, AHCI_BYTES_PER_PRDT, AHCI_PRDTS};
use crate::pci::{PCI, PCI_DRIVER_AHCI};
use crate::system::{LinkedListSearch, IS_ALIGNED};
use crate::malloc::{malloc, free};

pub const SECTOR_SIZE: usize = 512;
pub const MBR_PARTITION_1: usize = 446;
pub const MBR_PARTITION_2: usize = 462;
pub const MBR_PARTITION_3: usize = 478;
pub const MBR_PARTITION_4: usize = 494;

pub static MBR_PARTITION_INDEXES: [usize; 4] =
    [MBR_PARTITION_1, MBR_PARTITION_2, MBR_PARTITION_3, MBR_PARTITION_4];

#[repr(C)]
pub struct MbrPartition {
    pub boot_flag: u8,
    pub start_chs: [u8; 3],
    pub partition_type: u8,
    pub end_chs: [u8; 3],
    pub start_lba: u32,
    pub size: u32,
}

/// ---------------------------
/// Open a disk & read a partition
/// ---------------------------
pub fn open_disk(disk: u32, partition: usize, out: &mut MbrPartition) -> bool {
    unsafe {
        let raw_arr = malloc(SECTOR_SIZE) as *mut u8;
        if raw_arr.is_null() {
            return false;
        }

        get_disk_bytes(raw_arr, 0, 1);

        if !validate_mbr(core::slice::from_raw_parts(raw_arr, SECTOR_SIZE)) {
            free(raw_arr as *mut _);
            return false;
        }

        let partition_offset = MBR_PARTITION_INDEXES[partition];
        let src = raw_arr.add(partition_offset);
        copy_nonoverlapping(src, out as *mut _ as *mut u8, size_of::<MbrPartition>());
        free(raw_arr as *mut _);
        true
    }
}

/// ---------------------------
/// Validate MBR signature
/// ---------------------------
pub fn validate_mbr(mbr_sector: &[u8]) -> bool {
    mbr_sector[510] == 0x55 && mbr_sector[511] == 0xAA
}

/// ---------------------------
/// Disk callback for PCI scan
/// ---------------------------
unsafe fn disk_bytes_cb(data: *mut core::ffi::c_void, _ctx: *mut core::ffi::c_void) -> bool {
    let browse = data as *mut PCI;
    (*browse).driver == PCI_DRIVER_AHCI && ((*browse).extra as *mut ahci).as_ref().unwrap().sata != 0
}

/// ---------------------------
/// Read/write raw sectors
/// ---------------------------
pub fn disk_bytes(target_address: *mut u8, lba: u32, sector_count: usize, write: bool) {
    unsafe {
        let browse = LinkedListSearch(&crate::system::DS_PCI, Some(disk_bytes_cb), null_mut());
        if browse.is_null() {
            // zero memory if no AHCI disk found
            core::ptr::write_bytes(target_address, 0, sector_count * SECTOR_SIZE);
            return;
        }

        let target = (*browse).extra as *mut ahci;
        let mut pos = 0;
        while ((*target).sata & (1 << pos)) == 0 {
            pos += 1;
        }

        if write {
            ahci_write(target, pos, &mut (*target).mem.as_mut().unwrap().ports[pos], lba, 0, sector_count, target_address);
        } else {
            ahci_read(target, pos, &mut (*target).mem.as_mut().unwrap().ports[pos], lba, 0, sector_count, target_address);
        }
    }
}

/// ---------------------------
/// Disk bytes max (chunked for large transfers)
/// ---------------------------
#[inline(always)]
pub fn disk_bytes_max(target_address: *mut u8, lba: u32, sector_count: usize, write: bool) {
    let mut prdt_amount = AHCI_PRDTS;

    if !IS_ALIGNED(target_address as usize, 0x1000) {
        prdt_amount -= 1;
    }

    let max_sectors = (AHCI_BYTES_PER_PRDT * prdt_amount) / SECTOR_SIZE;

    let chunks = sector_count / max_sectors;
    let remainder = sector_count % max_sectors;

    for i in 0..chunks {
        unsafe {
            disk_bytes(
                target_address.add(i * max_sectors * SECTOR_SIZE),
                lba + (i as u32 * max_sectors as u32),
                max_sectors,
                write,
            );
        }
    }

    if remainder > 0 {
        unsafe {
            disk_bytes(
                target_address.add(chunks * max_sectors * SECTOR_SIZE),
                lba + (chunks as u32 * max_sectors as u32),
                remainder,
                write,
            );
        }
    }
}

/// ---------------------------
/// Helper functions
/// ---------------------------
pub fn get_disk_bytes(target_address: *mut u8, lba: u32, sector_count: usize) {
    disk_bytes_max(target_address, lba, sector_count, false);
}

pub fn set_disk_bytes(target_address: *const u8, lba: u32, sector_count: usize) {
    disk_bytes_max(target_address as *mut u8, lba, sector_count, true);
}
