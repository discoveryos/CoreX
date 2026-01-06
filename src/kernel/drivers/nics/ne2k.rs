#![no_std]

use core::ptr::null_mut;

//
// ================= Externs =================
//

extern "C" {
    fn debugf(fmt: *const u8, ...) -> i32;

    fn outportb(port: u16, value: u8);
    fn inportb(port: u16) -> u8;

    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);

    fn GetGeneralDevice(dev: *mut PCIdevice, out: *mut PCIgeneralDevice);
    fn createNewNIC(pci: *mut PCI) -> *mut NIC;

    static mut selectedNIC: *mut NIC;
}

//
// ================= Basic structs =================
//

#[repr(C)]
pub struct PCIdevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
}

#[repr(C)]
pub struct PCIgeneralDevice {
    pub bar: [u32; 6],
    pub interruptLine: u8,
}

#[repr(C)]
pub struct PCI {
    _unused: u8,
}

#[repr(C)]
pub struct NIC {
    pub r#type: u32,
    pub infoLocation: *mut ne2k_interface,
    pub MAC: [u8; 6],
}

#[repr(C)]
pub struct ne2k_interface {
    pub iobase: u16,
}

//
// ================= Constants =================
//

const NE2000_VENDOR: u16 = 0x10ec;
const NE2000_DEVICE: u16 = 0x8029;

const NE2000: u32 = 2;

const NE2K_REG_COMMAND: u16 = 0x00;
const NE2K_REG_DCR: u16 = 0x0E;
const NE2K_REG_RBCR0: u16 = 0x0A;
const NE2K_REG_RBCR1: u16 = 0x0B;
const NE2K_REG_IMR: u16 = 0x0F;
const NE2K_REG_ISR: u16 = 0x07;
const NE2K_REG_RCR: u16 = 0x0C;
const NE2K_REG_TCR: u16 = 0x0D;
const NE2K_REG_RSAR0: u16 = 0x08;
const NE2K_REG_RSAR1: u16 = 0x09;
const NE2K_REG_DATA: u16 = 0x10;

//
// ================= Detection =================
//

#[no_mangle]
pub extern "C" fn isNe2000(device: *const PCIdevice) -> bool {
    unsafe {
        (*device).vendor_id == NE2000_VENDOR &&
        (*device).device_id == NE2000_DEVICE
    }
}

//
// ================= Initialization =================
//

#[no_mangle]
pub extern "C" fn initiateNe2000(device: *mut PCIdevice) -> bool {
    unsafe {
        if !isNe2000(device) {
            return false;
        }

        debugf(b"[pci::ne2k] Ne2000 NIC detected!\n\0".as_ptr());

        // BUGGY_NE2k behavior preserved
        debugf(b"[pci::ne2k] Ignored!\n\0".as_ptr());
        return false;
    }

    #[allow(unreachable_code)]
    unsafe {
        let details =
            malloc(core::mem::size_of::<PCIgeneralDevice>()) as *mut PCIgeneralDevice;
        GetGeneralDevice(device, details);

        let iobase = ((*details).bar[0] & !0x3) as u16;

        // Reset
        outportb(iobase + 0x1F, inportb(iobase + 0x1F));
        while (inportb(iobase + NE2K_REG_ISR) & 0x80) == 0 {}
        outportb(iobase + NE2K_REG_ISR, 0xFF);

        outportb(iobase + NE2K_REG_COMMAND, (1 << 5) | 1);
        outportb(iobase + NE2K_REG_DCR, 0x49);
        outportb(iobase + NE2K_REG_RBCR0, 0);
        outportb(iobase + NE2K_REG_RBCR1, 0);
        outportb(iobase + NE2K_REG_IMR, 0);
        outportb(iobase + NE2K_REG_ISR, 0xFF);
        outportb(iobase + NE2K_REG_RCR, 0x20);
        outportb(iobase + NE2K_REG_TCR, 0x02);
        outportb(iobase + NE2K_REG_RBCR0, 32);
        outportb(iobase + NE2K_REG_RBCR1, 0);
        outportb(iobase + NE2K_REG_RSAR0, 0);
        outportb(iobase + NE2K_REG_RSAR1, 0);
        outportb(iobase + NE2K_REG_COMMAND, 0x0A);

        let nic = createNewNIC(null_mut());
        (*nic).r#type = NE2000;

        let info =
            malloc(core::mem::size_of::<ne2k_interface>()) as *mut ne2k_interface;
        (*info).iobase = iobase;
        (*nic).infoLocation = info;

        // Read PROM / MAC
        for i in 0..32 {
            let val = inportb(iobase + NE2K_REG_DATA);
            if i < 6 {
                (*nic).MAC[i] = val;
            }
        }

        // Program MAC
        for i in 0..6 {
            outportb(iobase + 1 + i as u16, (*selectedNIC).MAC[i]);
        }

        outportb(iobase + NE2K_REG_COMMAND, (1 << 5) | 1);

        free(details as *mut u8);
        true
    }
}

//
// ================= Transmit =================
//

#[no_mangle]
pub extern "C" fn sendNe2000(nic: *mut NIC, packet: *const u8, packetSize: u32) {
    unsafe {
        debugf(b"[pci::ne2k] Ignored!\n\0".as_ptr());

        let info = (*nic).infoLocation;
        let iobase = (*info).iobase;

        outportb(iobase + NE2K_REG_COMMAND, 0x22);
        outportb(iobase + NE2K_REG_RBCR0, (packetSize & 0xFF) as u8);
        outportb(iobase + NE2K_REG_RBCR1, (packetSize >> 8) as u8);
        outportb(iobase + NE2K_REG_ISR, 1 << 6);
        outportb(iobase + NE2K_REG_RSAR0, 0);
        outportb(iobase + NE2K_REG_RSAR1, 0);
        outportb(iobase + NE2K_REG_COMMAND, 0x12);

        for i in 0..packetSize {
            let byte = *packet.add(i as usize);
            outportb(iobase + NE2K_REG_DATA, byte);
        }

        while (inportb(iobase + NE2K_REG_ISR) & (1 << 6)) == 0 {}
    }
}
