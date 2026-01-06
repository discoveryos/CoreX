#![no_std]

use core::cmp::min;
use core::ptr::{copy_nonoverlapping, null_mut};

//
// ===== Extern kernel symbols =====
//

extern "C" {
    fn outportl(port: u16, value: u32);
    fn inportl(port: u16) -> u32;

    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);

    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn initiateNIC(dev: *const PCIdevice);
    fn initiateAHCI(dev: *const PCIdevice);
    fn initiateVMWareSvga2(dev: *const PCIdevice);

    static mut dsPCI: LinkedList;
}

//
// ===== Constants =====
//

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

const PCI_MAX_BUSES: u16 = 256;
const PCI_MAX_DEVICES: u8 = 32;
const PCI_MAX_FUNCTIONS: u8 = 8;

const PCI_DEVICE_GENERAL: u8 = 0x0;

const PCI_CLASS_CODE_NETWORK_CONTROLLER: u8 = 0x02;
const PCI_CLASS_CODE_MASS_STORAGE_CONTROLLER: u8 = 0x01;
const PCI_CLASS_CODE_DISPLAY_CONTROLLER: u8 = 0x03;

//
// PCI register offsets
//

const PCI_DEVICE_ID: u8 = 0x02;
const PCI_COMMAND: u8 = 0x04;
const PCI_STATUS: u8 = 0x06;
const PCI_REVISION_ID: u8 = 0x08;
const PCI_SUBCLASS: u8 = 0x0A;
const PCI_CACHE_LINE_SIZE: u8 = 0x0C;
const PCI_HEADER_TYPE: u8 = 0x0E;
const PCI_BAR0: u8 = 0x10;
const PCI_SYSTEM_VENDOR_ID: u8 = 0x2C;
const PCI_SYSTEM_ID: u8 = 0x2E;
const PCI_EXP_ROM_BASE_ADDR: u8 = 0x30;
const PCI_CAPABILITIES_PTR: u8 = 0x34;
const PCI_INTERRUPT_LINE: u8 = 0x3C;
const PCI_MIN_GRANT: u8 = 0x3E;
const PCI_PRIMARY_BUS_NUM: u8 = 0x18;

//
// ===== Helper macros (functions in Rust) =====
//

#[inline]
fn export_byte(word: u16, high: bool) -> u8 {
    if high {
        (word >> 8) as u8
    } else {
        (word & 0xFF) as u8
    }
}

#[inline]
fn combine_word(high: u16, low: u16) -> u32 {
    ((high as u32) << 16) | (low as u32)
}

//
// ===== Data structures =====
//

#[repr(C)]
pub struct PCIdevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,

    pub vendor_id: u16,
    pub device_id: u16,

    pub command: u16,
    pub status: u16,

    pub revision: u8,
    pub progIF: u8,

    pub subclass_id: u8,
    pub class_id: u8,

    pub cacheLineSize: u8,
    pub latencyTimer: u8,

    pub headerType: u8,
    pub bist: u8,
}

#[repr(C)]
pub struct PCIgeneralDevice {
    pub bar: [u32; 6],
    pub system_vendor_id: u16,
    pub system_id: u16,
    pub expROMaddr: u32,
    pub capabilitiesPtr: u8,
    pub interruptLine: u8,
    pub interruptPIN: u8,
    pub minGrant: u8,
    pub maxLatency: u8,
}

#[repr(C)]
pub struct PCI {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub driver: usize,
    pub category: usize,
}

#[repr(C)]
pub struct LinkedList;

extern "C" {
    fn LinkedListInit(list: *mut LinkedList, size: usize);
    fn LinkedListAllocate(list: *mut LinkedList, size: usize) -> *mut PCI;
    fn LinkedListSearch(
        list: *mut LinkedList,
        cb: extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> bool,
        ctx: *mut core::ffi::c_void,
    ) -> *mut PCI;
}

//
// ===== PCI abstraction =====
//

#[no_mangle]
pub extern "C" fn lookupPCIdeviceCb(
    data: *mut core::ffi::c_void,
    ctx: *mut core::ffi::c_void,
) -> bool {
    unsafe {
        let browse = data as *mut PCI;
        let device = ctx as *mut PCIdevice;

        (*browse).bus == (*device).bus
            && (*browse).slot == (*device).slot
            && (*browse).function == (*device).function
    }
}

pub unsafe fn lookupPCIdevice(device: *mut PCIdevice) -> *mut PCI {
    LinkedListSearch(&mut dsPCI, lookupPCIdeviceCb, device as *mut _)
}

pub unsafe fn setupPCIdeviceDriver(
    pci: *mut PCI,
    driver: usize,
    category: usize,
) {
    (*pci).driver = driver;
    (*pci).category = category;
}

//
// ===== PCI config I/O =====
//

pub unsafe fn config_read_word(
    bus: u8,
    slot: u8,
    func: u8,
    offset: u8,
) -> u16 {
    let address: u32 =
        ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x8000_0000;

    outportl(PCI_CONFIG_ADDRESS, address);
    ((inportl(PCI_CONFIG_DATA) >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

pub unsafe fn config_write_dword(
    bus: u8,
    slot: u8,
    func: u8,
    offset: u8,
    value: u32,
) {
    let address: u32 =
        ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x8000_0000;

    outportl(PCI_CONFIG_ADDRESS, address);
    outportl(PCI_CONFIG_DATA, value);
}

//
// ===== Device enumeration helpers =====
//

pub unsafe fn filter_device(bus: u8, slot: u8, function: u8) -> bool {
    let vendor = config_read_word(bus, slot, function, 0x00);
    !(vendor == 0xFFFF || vendor == 0)
}

//
// ===== Device info population =====
//

pub unsafe fn get_device(
    dev: *mut PCIdevice,
    bus: u8,
    slot: u8,
    function: u8,
) {
    (*dev).bus = bus;
    (*dev).slot = slot;
    (*dev).function = function;

    (*dev).vendor_id = config_read_word(bus, slot, function, 0x00);
    (*dev).device_id = config_read_word(bus, slot, function, PCI_DEVICE_ID);

    (*dev).command = config_read_word(bus, slot, function, PCI_COMMAND);
    (*dev).status = config_read_word(bus, slot, function, PCI_STATUS);

    let rev_prog = config_read_word(bus, slot, function, PCI_REVISION_ID);
    (*dev).revision = export_byte(rev_prog, true);
    (*dev).progIF = export_byte(rev_prog, false);

    let sub_class = config_read_word(bus, slot, function, PCI_SUBCLASS);
    (*dev).subclass_id = export_byte(sub_class, true);
    (*dev).class_id = export_byte(sub_class, false);

    let cache_lat = config_read_word(bus, slot, function, PCI_CACHE_LINE_SIZE);
    (*dev).cacheLineSize = export_byte(cache_lat, true);
    (*dev).latencyTimer = export_byte(cache_lat, false);

    let header_bist = config_read_word(bus, slot, function, PCI_HEADER_TYPE);
    (*dev).headerType = export_byte(header_bist, true);
    (*dev).bist = export_byte(header_bist, false);
}

//
// ===== PCI init =====
//

pub unsafe fn initiate_pci() {
    let device = malloc(core::mem::size_of::<PCIdevice>()) as *mut PCIdevice;
    LinkedListInit(&mut dsPCI, core::mem::size_of::<PCI>());

    for bus in 0..PCI_MAX_BUSES {
        for slot in 0..PCI_MAX_DEVICES {
            for function in 0..PCI_MAX_FUNCTIONS {
                if !filter_device(bus as u8, slot, function) {
                    continue;
                }

                get_device(device, bus as u8, slot, function);

                if ((*device).headerType & !(1 << 7)) != PCI_DEVICE_GENERAL {
                    continue;
                }

                let target =
                    LinkedListAllocate(&mut dsPCI, core::mem::size_of::<PCI>());
                (*target).bus = bus as u8;
                (*target).slot = slot;
                (*target).function = function;
                (*target).vendor_id = (*device).vendor_id;
                (*target).device_id = (*device).device_id;

                match (*device).class_id {
                    PCI_CLASS_CODE_NETWORK_CONTROLLER => {
                        initiateNIC(device);
                    }
                    PCI_CLASS_CODE_MASS_STORAGE_CONTROLLER => {
                        if (*device).subclass_id == 0x06 {
                            initiateAHCI(device);
                        }
                    }
                    PCI_CLASS_CODE_DISPLAY_CONTROLLER => {
                        initiateVMWareSvga2(device);
                    }
                    _ => {}
                }
            }
        }
    }

    free(device as *mut u8);
}
