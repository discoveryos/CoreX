#![no_std]

use core::ptr::null_mut;

//
// ================= Externs =================
//

extern "C" {
    fn debugf(fmt: *const u8, ...) -> i32;

    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    fn memcpy(dst: *mut u8, src: *const u8, len: usize);
    fn memset(dst: *mut u8, val: i32, len: usize);

    fn inportb(port: u16) -> u8;
    fn inportw(port: u16) -> u16;
    fn inportl(port: u16) -> u32;
    fn outportb(port: u16, val: u8);
    fn outportw(port: u16, val: u16);
    fn outportl(port: u16, val: u32);

    fn lookupPCIdevice(dev: *mut PCIdevice) -> *mut PCI;
    fn setupPCIdeviceDriver(pci: *mut PCI, driver: u32, category: u32);
    fn GetGeneralDevice(dev: *mut PCIdevice, out: *mut PCIgeneralDevice);
    fn ConfigWriteDword(bus: u8, slot: u8, func: u8, reg: u8, val: u32);

    fn ioApicPciRegister(dev: *mut PCIdevice, info: *mut PCIgeneralDevice) -> u8;
    fn registerIRQhandler(irq: u8, handler: extern "C" fn(*mut AsmPassedInterrupt))
        -> *mut core::ffi::c_void;

    fn VirtualAllocatePhysicallyContiguous(blocks: usize) -> *mut u8;
    fn VirtualFree(ptr: *mut u8, blocks: usize);
    fn VirtualToPhysical(addr: usize) -> usize;

    fn DivRoundUp(val: usize, div: usize) -> usize;

    fn createNewNIC(pci: *mut PCI) -> *mut NIC;
    fn netQueueAdd(nic: *mut NIC, data: *const u8, len: u16);

    static mut selectedNIC: *mut NIC;
}

//
// ================= Constants =================
//

pub const RTL8139: u32 = 2;
pub const PCI_DRIVER_RTL8139: u32 = 2;
pub const PCI_DRIVER_CATEGORY_NIC: u32 = 2;

pub const BLOCK_SIZE: usize = 4096;
pub const UINT32_MAX: usize = 0xFFFF_FFFF;

pub const RTL8139_REG_CMD: u16 = 0x37;
pub const RTL8139_REG_ISR: u16 = 0x3E;
pub const RTL8139_REG_IMR: u16 = 0x3C;
pub const RTL8139_REG_RBSTART: u16 = 0x30;
pub const RTL8139_REG_MAC0_5: u16 = 0x00;
pub const RTL8139_REG_MAC5_6: u16 = 0x04;
pub const RTL8139_REG_POWERUP: u16 = 0x52;

pub const RTL8139_STATUS_TOK: u16 = 1 << 2;
pub const RTL8139_STATUS_ROK: u16 = 1 << 0;

//
// ================= Globals =================
//

static TSAD_ARRAY: [u16; 4] = [0x20, 0x24, 0x28, 0x2C];
static TSD_ARRAY: [u16; 4] = [0x10, 0x14, 0x18, 0x1C];

static mut LOCK_RTL8139: Spinlock = Spinlock { locked: 0 };

//
// ================= Structs =================
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
}

#[repr(C)]
pub struct PCIgeneralDevice {
    pub bar: [u32; 6],
    pub interruptLine: u8,
}

#[repr(C)]
pub struct PCI {
    pub irqHandler: *mut core::ffi::c_void,
    pub extra: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct NIC {
    pub r#type: u32,
    pub mtu: u16,
    pub mintu: u16,
    pub MAC: [u8; 6],
    pub infoLocation: *mut core::ffi::c_void,
    pub irq: u8,
}

#[repr(C)]
pub struct rtl8139_interface {
    pub iobase: u16,
    pub rx_buff_virtual: *mut u8,
    pub currentPacket: u32,
    pub tx_curr: u8,
    pub tok: u8,
}

#[repr(C)]
pub struct Spinlock {
    pub locked: u32,
}

#[repr(C)]
pub struct AsmPassedInterrupt {
    _unused: u8,
}

//
// ================= Helpers =================
//

fn spinlock_acquire(lock: &mut Spinlock) {
    while core::sync::atomic::AtomicU32::new(lock.locked)
        .compare_exchange(0, 1, core::sync::atomic::Ordering::Acquire,
                          core::sync::atomic::Ordering::Relaxed)
        .is_err()
    {}
}

fn spinlock_release(lock: &mut Spinlock) {
    lock.locked = 0;
}

//
// ================= Detection =================
//

#[no_mangle]
pub extern "C" fn isRTL8139(device: *mut PCIdevice) -> bool {
    unsafe {
        (*device).vendor_id == 0x10ec && (*device).device_id == 0x8139
    }
}

//
// ================= IRQ Handler =================
//

#[no_mangle]
pub extern "C" fn interruptHandler(_regs: *mut AsmPassedInterrupt) {
    unsafe {
        let info = (*selectedNIC).infoLocation as *mut rtl8139_interface;
        let iobase = (*info).iobase;

        loop {
            let status = inportw(iobase + RTL8139_REG_ISR);
            outportw(iobase + RTL8139_REG_ISR, 0x5);

            if status == 0 {
                break;
            }

            if status & RTL8139_STATUS_TOK != 0 {
                for i in 0..4 {
                    let v = inportl(iobase + TSD_ARRAY[i]);
                    if v & (1 << 15) != 0 {
                        (*info).tok |= 1 << i;
                    }
                }
            }

            if status & RTL8139_STATUS_ROK != 0 {
                receiveRTL8139(selectedNIC);
            }

            if status & (RTL8139_STATUS_TOK | RTL8139_STATUS_ROK) == 0 {
                break;
            }
        }
    }
}

//
// ================= Init =================
//

#[no_mangle]
pub extern "C" fn initiateRTL8139(device: *mut PCIdevice) -> bool {
    unsafe {
        if !isRTL8139(device) {
            return false;
        }

        debugf(b"[pci::rtl8139] RTL-8139 NIC detected!\n\0".as_ptr());

        let details = malloc(core::mem::size_of::<PCIgeneralDevice>())
            as *mut PCIgeneralDevice;
        GetGeneralDevice(device, details);

        let pci = lookupPCIdevice(device);
        setupPCIdeviceDriver(pci, PCI_DRIVER_RTL8139, PCI_DRIVER_CATEGORY_NIC);

        let iobase = ((*details).bar[0] & !0x3) as u16;

        let nic = createNewNIC(pci);
        (*nic).r#type = RTL8139;
        (*nic).mintu = 60;
        (*nic).irq = (*details).interruptLine;

        let info = malloc(core::mem::size_of::<rtl8139_interface>())
            as *mut rtl8139_interface;
        memset(info as *mut u8, 0, core::mem::size_of::<rtl8139_interface>());
        (*nic).infoLocation = info;
        (*info).iobase = iobase;

        outportb(iobase + RTL8139_REG_POWERUP, 0);
        outportb(iobase + RTL8139_REG_CMD, 0x10);
        while inportb(iobase + RTL8139_REG_CMD) & 0x10 != 0 {}

        let rx_size = 8192 + 16 + 1500;
        let virt = VirtualAllocatePhysicallyContiguous(
            DivRoundUp(rx_size, BLOCK_SIZE),
        );
        memset(virt, 0, rx_size);
        let phys = VirtualToPhysical(virt as usize);
        outportl(iobase + RTL8139_REG_RBSTART, phys as u32);

        (*info).rx_buff_virtual = virt;

        outportw(iobase + RTL8139_REG_IMR, 0x0005);
        outportb(iobase + RTL8139_REG_CMD, 0x0C);
        outportl(iobase + 0x44, 0xF | (1 << 7));

        let mac0 = inportl(iobase + RTL8139_REG_MAC0_5);
        let mac1 = inportw(iobase + RTL8139_REG_MAC5_6);

        (*nic).MAC = [
            mac0 as u8,
            (mac0 >> 8) as u8,
            (mac0 >> 16) as u8,
            (mac0 >> 24) as u8,
            mac1 as u8,
            (mac1 >> 8) as u8,
        ];

        let irq = ioApicPciRegister(device, details);
        free(details as *mut u8);
        (*pci).irqHandler = registerIRQhandler(irq, interruptHandler);

        true
    }
}

//
// ================= TX =================
//

#[no_mangle]
pub extern "C" fn sendRTL8139(nic: *mut NIC, packet: *const u8, size: u32) {
    unsafe {
        spinlock_acquire(&mut LOCK_RTL8139);

        let info = (*nic).infoLocation as *mut rtl8139_interface;
        let iobase = (*info).iobase;

        let blocks = DivRoundUp(size as usize, BLOCK_SIZE);
        let virt = VirtualAllocatePhysicallyContiguous(blocks);
        let phys = VirtualToPhysical(virt as usize);

        if phys > UINT32_MAX - 0x5000 {
            VirtualFree(virt, blocks);
            spinlock_release(&mut LOCK_RTL8139);
            return;
        }

        memcpy(virt, packet, size as usize);

        let idx = (*info).tx_curr as usize;
        outportl(iobase + TSAD_ARRAY[idx], phys as u32);
        outportl(iobase + TSD_ARRAY[idx], size);

        (*info).tx_curr = ((*info).tx_curr + 1) & 3;

        while inportl(iobase + TSD_ARRAY[idx]) & (1 << 15) == 0 {}

        outportl(iobase + TSD_ARRAY[idx], 0x2000);
        VirtualFree(virt, blocks);
        spinlock_release(&mut LOCK_RTL8139);
    }
}

//
// ================= RX =================
//

#[no_mangle]
pub extern "C" fn receiveRTL8139(nic: *mut NIC) {
    unsafe {
        let info = (*nic).infoLocation as *mut rtl8139_interface;
        let iobase = (*info).iobase;

        while inportb(iobase + 0x37) & 0x01 == 0 {
            let base = (*info).rx_buff_virtual.add((*info).currentPacket as usize)
                as *mut u16;

            let status = *base;
            let len = *base.add(1);

            if status == 0 || status == 0xe1e3 {
                debugf(b"[pci::rtl8139] Bad packet!\n\0".as_ptr());
                return;
            }

            let payload = base.add(2) as *const u8;
            netQueueAdd(nic, payload, len - 4);

            (*info).currentPacket =
                ((*info).currentPacket + len as u32 + 4 + 3) & !3;

            if (*info).currentPacket >= 8192 {
                (*info).currentPacket -= 8192;
            }

            outportw(iobase + 0x38, (*info).currentPacket as u16 - 0x10);
        }
    }
}
