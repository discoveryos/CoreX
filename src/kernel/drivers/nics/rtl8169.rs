#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};
use core::slice;

use crate::pci::*;
use crate::system::*;
use crate::net::*;
use crate::bootloader::*;

pub const RTL8169_RX_DESCRIPTORS: usize = 128;
pub const RTL8169_TX_DESCRIPTORS: usize = 8;

pub const RTL8169_OWN: u32 = 0x80000000;
pub const RTL8169_EOR: u32 = 0x40000000;

pub const RTL8169_LINK_CHANGE: u16 = 1 << 2;
pub const RTL8169_RECV: u16 = 1 << 0;
pub const RTL8169_SENT: u16 = 1 << 1;

#[repr(C)]
pub struct Rtl8169Descriptor {
    pub low_buf: u32,
    pub high_buf: u32,
    pub command: u32,
    pub vlan: u32,
}

pub struct Rtl8169Interface {
    pub iobase: u16,
    pub rx_descriptors: *mut Rtl8169Descriptor,
    pub tx_descriptors: *mut Rtl8169Descriptor,
    pub tx_sent: bool,
}

pub struct Nic {
    pub mac: [u8; 6],
    pub irq: u8,
    pub info_location: *mut Rtl8169Interface,
}

pub static mut SELECTED_NIC: Option<&'static mut Nic> = None;

fn inportb(port: u16) -> u8 {
    unsafe {
        let value: u8;
        core::arch::asm!("in al, dx", out("al") value, in("dx") port);
        value
    }
}

fn outportb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value);
    }
}

fn inportw(port: u16) -> u16 {
    unsafe {
        let value: u16;
        core::arch::asm!("in ax, dx", out("ax") value, in("dx") port);
        value
    }
}

fn outportw(port: u16, value: u16) {
    unsafe {
        core::arch::asm!("out dx, ax", in("dx") port, in("ax") value);
    }
}

fn inportl(port: u16) -> u32 {
    unsafe {
        let value: u32;
        core::arch::asm!("in eax, dx", out("eax") value, in("dx") port);
        value
    }
}

fn outportl(port: u16, value: u32) {
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") value);
    }
}

pub fn is_rtl8169(device: &PciDevice) -> bool {
    (device.vendor_id == 0x10ec
        && (device.device_id == 0x8161
            || device.device_id == 0x8168
            || device.device_id == 0x8169))
        || (device.vendor_id == 0x1259 && device.device_id == 0xc107)
        || (device.vendor_id == 0x1737 && device.device_id == 0x1032)
        || (device.vendor_id == 0x16ec && device.device_id == 0x0116)
}

pub extern "C" fn interrupt_handler(_regs: &AsmPassedInterrupt) {
    unsafe {
        if let Some(nic) = SELECTED_NIC.as_mut() {
            let info = &mut *nic.info_location;
            let status = inportw(info.iobase + 0x3E);

            if status & RTL8169_RECV != 0 {
                for i in 0..RTL8169_RX_DESCRIPTORS {
                    let desc = &mut *info.rx_descriptors.add(i);
                    if desc.command & RTL8169_OWN != 0 {
                        continue;
                    }

                    let buff_size = desc.command & 0x3FFF;
                    let phys = ((desc.high_buf as u64) << 32) | desc.low_buf as u64;
                    let virt = phys + bootloader().hhdm_offset;

                    net_queue_add(nic, virt as *const u8, buff_size as usize - 4);

                    let is_final = desc.command & RTL8169_EOR != 0;
                    desc.command = RTL8169_OWN | (1536 & 0x3FFF);
                    if is_final {
                        desc.command |= RTL8169_EOR;
                    }
                }
                outportw(info.iobase + 0x3E, status | RTL8169_RECV);
            }

            if status & RTL8169_SENT != 0 {
                info.tx_sent = true;
                outportw(info.iobase + 0x3E, status | RTL8169_SENT);
            }
        }
    }
}

pub fn send_rtl8169(nic: &mut Nic, packet: &[u8]) {
    unsafe {
        let info = &mut *nic.info_location;
        let desc = &mut *info.tx_descriptors;

        let phys = ((desc.high_buf as u64) << 32) | desc.low_buf as u64;
        let virt = phys + bootloader().hhdm_offset;

        let slice = slice::from_raw_parts_mut(virt as *mut u8, packet.len());
        slice.copy_from_slice(packet);

        desc.vlan = 0;
        desc.command = RTL8169_OWN | RTL8169_EOR | ((packet.len() as u32) & 0x3FFF);

        info.tx_sent = false;
        outportb(info.iobase + 0x38, 0x40);

        while !info.tx_sent && (inportb(info.iobase + 0x38) & 0x40) != 0 {}
        info.tx_sent = false;
    }
}

pub fn setup_descriptors(info: &mut Rtl8169Interface) {
    unsafe {
        for i in 0..RTL8169_RX_DESCRIPTORS {
            let rx_buf = alloc_phys_contiguous(1536);
            let tx_buf = alloc_phys_contiguous(1536);

            let rx_phys = rx_buf as usize;
            let tx_phys = tx_buf as usize;

            let rx_desc = &mut *info.rx_descriptors.add(i);
            rx_desc.low_buf = rx_phys as u32;
            rx_desc.high_buf = (rx_phys >> 32) as u32;
            rx_desc.command = RTL8169_OWN | (1536 & 0x3FFF);
            if i == RTL8169_RX_DESCRIPTORS - 1 {
                rx_desc.command |= RTL8169_EOR;
            }

            let tx_desc = &mut *info.tx_descriptors.add(i);
            tx_desc.low_buf = tx_phys as u32;
            tx_desc.high_buf = (tx_phys >> 32) as u32;
            tx_desc.command = if i == RTL8169_TX_DESCRIPTORS - 1 { RTL8169_EOR } else { 0 };
            tx_desc.vlan = 0;
        }
    }
}

pub fn initiate_rtl8169(device: &PciDevice) -> Option<&'static mut Nic> {
    if !is_rtl8169(device) {
        return None;
    }

    let pci = lookup_pci_device(device);
    let iobase = (device.bar[0] & 0xFFFFFFFE) as u16;

    let nic = Box::leak(Box::new(Nic {
        mac: [0u8; 6],
        irq: device.interrupt_line,
        info_location: core::ptr::null_mut(),
    }));

    let info = Box::leak(Box::new(Rtl8169Interface {
        iobase,
        rx_descriptors: alloc_phys_contiguous(RTL8169_RX_DESCRIPTORS * core::mem::size_of::<Rtl8169Descriptor>()) as *mut _,
        tx_descriptors: alloc_phys_contiguous(RTL8169_TX_DESCRIPTORS * core::mem::size_of::<Rtl8169Descriptor>()) as *mut _,
        tx_sent: false,
    }));

    nic.info_location = info;

    // Read MAC
    for i in 0..6 {
        nic.mac[i] = inportb(iobase + i as u16);
    }

    setup_descriptors(info);

    // Enable interrupts
    let irq = io_apic_register(device);
    register_irq_handler(irq, interrupt_handler);

    unsafe { SELECTED_NIC = Some(nic) };
    Some(nic)
}
