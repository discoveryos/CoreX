#![no_std]

use core::ptr::{read_volatile, write_volatile};
use core::mem::size_of;
use core::sync::atomic::{AtomicU32, Ordering};

/* ============================================================
 * Constants
 * ============================================================ */

const ATA_CMD_READ_DMA_EX: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;

const ATA_DEV_BUSY: u32 = 0x80;
const ATA_DEV_DRQ:  u32 = 0x08;

const HBA_PxCMD_ST:  u32 = 1 << 0;
const HBA_PxCMD_FRE: u32 = 1 << 4;
const HBA_PxCMD_FR:  u32 = 1 << 14;
const HBA_PxCMD_CR:  u32 = 1 << 15;

const AHCI_PRDTS: usize = 8;
const AHCI_BYTES_PER_PRDT: usize = 4 * 1024 * 1024;

/* ============================================================
 * AHCI MMIO Structures
 * ============================================================ */

#[repr(C)]
pub struct HbaMem {
    pub cap: u32,
    pub ghc: u32,
    pub is: u32,
    pub pi: u32,
    pub vs: u32,
    pub ccc_ctl: u32,
    pub ccc_pts: u32,
    pub em_loc: u32,
    pub em_ctl: u32,
    pub cap2: u32,
    pub bohc: u32,
    _rsv: [u8; 0xA0 - 0x2C],
    pub ports: [HbaPort; 32],
}

#[repr(C)]
pub struct HbaPort {
    pub clb: u32,
    pub clbu: u32,
    pub fb: u32,
    pub fbu: u32,
    pub is: u32,
    pub ie: u32,
    pub cmd: u32,
    _rsv0: u32,
    pub tfd: u32,
    pub sig: u32,
    pub ssts: u32,
    pub serr: u32,
    pub sact: u32,
    pub ci: u32,
    _rsv1: [u32; 11],
}

/* ============================================================
 * Command Structures
 * ============================================================ */

#[repr(C)]
pub struct HbaCmdHeader {
    pub cfl: u8,
    pub w: u8,
    pub prdtl: u16,
    pub prdbc: u32,
    pub ctba: u32,
    pub ctbau: u32,
    _rsv: [u32; 4],
}

#[repr(C)]
pub struct HbaPrdtEntry {
    pub dba: u32,
    pub dbau: u32,
    _rsv0: u32,
    pub dbc: u32,
}

#[repr(C)]
pub struct HbaCmdTbl {
    pub cfis: [u8; 64],
    pub acmd: [u8; 16],
    pub _rsv: [u8; 48],
    pub prdt_entry: [HbaPrdtEntry; AHCI_PRDTS],
}

#[repr(C)]
pub struct FisRegH2d {
    pub fis_type: u8,
    pub pm: u8,
    pub command: u8,
    pub featurel: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub featureh: u8,
    pub countl: u8,
    pub counth: u8,
    pub icc: u8,
    pub control: u8,
    _rsv: [u8; 4],
}

/* ============================================================
 * Command Slot Tracking
 * ============================================================ */

static CMD_SLOTS_PREPARING: AtomicU32 = AtomicU32::new(0);

/* ============================================================
 * Command Engine Control
 * ============================================================ */

unsafe fn ahci_cmd_start(port: &mut HbaPort) {
    while read_volatile(&port.cmd) & HBA_PxCMD_CR != 0 {}
    write_volatile(
        &mut port.cmd,
        read_volatile(&port.cmd) | HBA_PxCMD_FRE | HBA_PxCMD_ST,
    );
}

unsafe fn ahci_cmd_stop(port: &mut HbaPort) {
    write_volatile(&mut port.cmd, read_volatile(&port.cmd) & !HBA_PxCMD_ST);
    write_volatile(&mut port.cmd, read_volatile(&port.cmd) & !HBA_PxCMD_FRE);

    loop {
        let v = read_volatile(&port.cmd);
        if v & (HBA_PxCMD_FR | HBA_PxCMD_CR) == 0 {
            break;
        }
    }
}

/* ============================================================
 * Utilities
 * ============================================================ */

unsafe fn ahci_port_ready(port: &HbaPort) -> bool {
    let mut timeout = 1_000_000;
    while read_volatile(&port.tfd) & (ATA_DEV_BUSY | ATA_DEV_DRQ) != 0 {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }
    true
}

fn find_cmd_slot(port: &HbaPort) -> Option<u8> {
    let used = unsafe { read_volatile(&port.sact) | read_volatile(&port.ci) };
    let mut prep = CMD_SLOTS_PREPARING.load(Ordering::Relaxed);

    for i in 0..32 {
        if (used & (1 << i)) == 0 && (prep & (1 << i)) == 0 {
            prep |= 1 << i;
            CMD_SLOTS_PREPARING.store(prep, Ordering::Relaxed);
            return Some(i);
        }
    }
    None
}

/* ============================================================
 * Read / Write
 * ============================================================ */

unsafe fn ahci_rw(
    port: &mut HbaPort,
    cmd_header: *mut HbaCmdHeader,
    cmd_tbl: *mut HbaCmdTbl,
    lba: u64,
    count: u16,
    buf_phys: u64,
    write: bool,
) -> bool {
    write_volatile(&mut port.is, 0xFFFF_FFFF);

    let hdr = &mut *cmd_header;
    hdr.cfl = (size_of::<FisRegH2d>() / 4) as u8;
    hdr.w = write as u8;
    hdr.prdtl = 1;

    let tbl = &mut *cmd_tbl;
    tbl.prdt_entry[0].dba  = buf_phys as u32;
    tbl.prdt_entry[0].dbau = (buf_phys >> 32) as u32;
    tbl.prdt_entry[0].dbc  = (count as u32 * 512) - 1;

    let fis = &mut *(tbl.cfis.as_mut_ptr() as *mut FisRegH2d);
    fis.fis_type = 0x27;
    fis.command  = if write { ATA_CMD_WRITE_DMA_EX } else { ATA_CMD_READ_DMA_EX };
    fis.device   = 1 << 6;

    fis.lba0 = lba as u8;
    fis.lba1 = (lba >> 8) as u8;
    fis.lba2 = (lba >> 16) as u8;
    fis.lba3 = (lba >> 24) as u8;
    fis.lba4 = (lba >> 32) as u8;
    fis.lba5 = (lba >> 40) as u8;

    fis.countl = count as u8;
    fis.counth = (count >> 8) as u8;

    if !ahci_port_ready(port) {
        return false;
    }

    write_volatile(&mut port.ci, 1);
    while read_volatile(&port.ci) & 1 != 0 {}

    true
}

/* ============================================================
 * Interrupt Handler
 * ============================================================ */

pub unsafe fn ahci_irq(mem: &mut HbaMem) {
    for port in mem.ports.iter_mut() {
        if read_volatile(&port.is) & (1 << 30) != 0 {
            panic!("AHCI task file error");
        }
        write_volatile(&mut port.is, read_volatile(&port.is));
    }
    write_volatile(&mut mem.is, read_volatile(&mem.is));
}
