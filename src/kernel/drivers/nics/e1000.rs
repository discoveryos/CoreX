#![no_std]

use core::ptr::{read_volatile, write_volatile};

//
// ================= Externs =================
//

extern "C" {
    fn outportl(port: usize, value: u32);
    fn inportl(port: usize) -> u32;
    fn debugf(fmt: *const u8, ...) -> i32;
    fn panic() -> !;
    fn handControl();

    fn VirtualAllocate(pages: usize) -> *mut u8;
    fn VirtualAllocatePhysicallyContiguous(pages: usize) -> *mut u8;
    fn VirtualToPhysical(addr: usize) -> usize;

    fn memset(ptr: *mut u8, val: i32, size: usize);
    fn memcpy(dst: *mut u8, src: *const u8, size: usize);

    static bootloader: BootloaderInfo;
    static mut selectedNIC: *mut NIC;
}

//
// ================= Basic structs =================
//

#[repr(C)]
pub struct BootloaderInfo {
    pub hhdmOffset: usize,
}

#[repr(C)]
pub struct PCIdevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
}

#[repr(C)]
pub struct NIC {
    pub infoLocation: *mut E1000Interface,
    pub MAC: [u8; 6],
    pub irq: u8,
    pub mintu: u16,
    pub r#type: u32,
}

//
// ================= E1000 structs =================
//

#[repr(C)]
pub struct E1000Interface {
    pub membase: usize,
    pub membasePhys: usize,
    pub iobase: usize,

    pub deviceId: u16,
    pub eeprom: u32,

    pub rxList: *mut E1000RX,
    pub txList: *mut E1000TX,

    pub rxHead: usize,
    pub nic: *mut NIC,
}

#[repr(C)]
pub struct E1000RX {
    pub addr: usize,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

#[repr(C)]
pub struct E1000TX {
    pub addr: usize,
    pub length: u16,
    pub cso: u8,
    pub command: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

//
// ================= Constants =================
//

const REG_EECD: u16 = 0x10;
const REG_EEPROM: u16 = 0x14;
const REG_CTRL: u16 = 0x00;
const REG_ICR: u16 = 0xC0;
const REG_IMASK: u16 = 0xD0;
const REG_IMASK_CLEAR: u16 = 0xD8;

const REG_RXDESCLO: u16 = 0x2800;
const REG_RXDESCHI: u16 = 0x2804;
const REG_RXDESCLEN: u16 = 0x2808;
const REG_RXDESCHEAD: u16 = 0x2810;
const REG_RXDESCTAIL: u16 = 0x2818;
const REG_RX_CONTROL: u16 = 0x0100;

const REG_TXDESCLO: u16 = 0x3800;
const REG_TXDESCHI: u16 = 0x3804;
const REG_TXDESCLEN: u16 = 0x3808;
const REG_TXDESCHEAD: u16 = 0x3810;
const REG_TXDESCTAIL: u16 = 0x3818;
const REG_TCTL: u16 = 0x0400;

const REG_RAL_BEGIN: usize = 0x5400;

const EECD_EEPROM_PRESENT: u32 = 1 << 8;
const EERD_START: u32 = 1;
const EERD_DONE: u32 = 1 << 4;
const EERD_DONE_EXTRA: u32 = 1 << 1;

const CTRL_SET_LINK_UP: u32 = 1 << 6;
const CTRL_LINK_RESET: u32 = 1 << 3;
const CTRL_PHY_RESET: u32 = 1 << 31;
const CTRL_VLAN_MODE_ENABLE: u32 = 1 << 30;
const CTRL_INVERT_LOSS_OF_SIGNAL: u32 = 1 << 7;

const CMD
