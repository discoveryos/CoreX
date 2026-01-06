#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};
use core::slice;

use crate::pci::*;
use crate::system::*;
use crate::fb::*;
use crate::bootloader::*;
use crate::util::*;
use crate::vga::*;

#[repr(C)]
pub struct VMWareSvga {
    pub base: usize,
    pub fifo: *mut u32,
    pub exists: bool,
}

pub static mut VMWareSvga2: VMWareSvga = VMWareSvga { base: 0, fifo: core::ptr::null_mut(), exists: false };

fn inportl(port: u16) -> u32 {
    let val: u32;
    unsafe { core::arch::asm!("in eax, dx", out("eax") val, in("dx") port); }
    val
}

fn outportl(port: u16, value: u32) {
    unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") value); }
}

pub fn vmware_svga2_detect(device: &PciDevice) -> bool {
    device.vendor_id == 0x15ad && device.device_id == 0x0405
}

pub fn vmware_svga2_read(index: u32) -> u32 {
    unsafe {
        outportl(VMWareSvga2.base as u16 + SVGA_INDEX, index);
        inportl(VMWareSvga2.base as u16 + SVGA_VALUE)
    }
}

pub fn vmware_svga2_write(index: u32, value: u32) {
    unsafe {
        outportl(VMWareSvga2.base as u16 + SVGA_INDEX, index);
        outportl(VMWareSvga2.base as u16 + SVGA_VALUE, value);
    }
}

pub fn vmware_svga2_set_mode(width: u32, height: u32, bpp: u32) {
    assert!(width <= vmware_svga2_read(SVGA_REG_MAX_WIDTH));
    assert!(height <= vmware_svga2_read(SVGA_REG_MAX_HEIGHT));

    vmware_svga2_write(SVGA_REG_WIDTH, width);
    vmware_svga2_write(SVGA_REG_HEIGHT, height);
    vmware_svga2_write(SVGA_REG_BPP, bpp);

    assert!(vmware_svga2_read(SVGA_REG_WIDTH) == width);
    assert!(vmware_svga2_read(SVGA_REG_HEIGHT) == height);
    assert!(vmware_svga2_read(SVGA_REG_BPP) == bpp);

    // optional: read extra config registers
    let _ = vmware_svga2_read(SVGA_REG_BYTES_PER_LINE);
    let _ = vmware_svga2_read(SVGA_REG_DEPTH);
    let _ = vmware_svga2_read(SVGA_REG_PSEUDOCOLOR);
    let _ = vmware_svga2_read(SVGA_REG_RED_MASK);
    let _ = vmware_svga2_read(SVGA_REG_GREEN_MASK);
    let _ = vmware_svga2_read(SVGA_REG_BLUE_MASK);
}

pub fn vmware_svga2_sync() {
    unsafe {
        let phys = vmware_svga2_read(SVGA_REG_FB_START) as usize
            + vmware_svga2_read(SVGA_REG_FB_OFFSET) as usize;
        let addr = 0x5000_0000_0000usize; // TODO: choose proper virtual mapping
        let pages = div_round_up(vmware_svga2_read(SVGA_REG_VRAM_SIZE) as usize, PAGE_SIZE);

        for i in 0..pages {
            virtual_map(addr + i * PAGE_SIZE, phys + i * PAGE_SIZE, PF_RW | PF_CACHE_WC);
        }

        fb().virt = addr as *mut u8;
        fb().phys = phys;
        fb().width = vmware_svga2_read(SVGA_REG_WIDTH);
        fb().height = vmware_svga2_read(SVGA_REG_HEIGHT);
        fb().pitch = fb().width * 4;

        let bpp = vmware_svga2_read(SVGA_REG_BPP);
        assert!(bpp == 32);
    }
}

fn vmware_svga2_fifo_write(value: u32) {
    unsafe {
        let fifo = &mut *VMWareSvga2.fifo;
        let mut next_cmd = fifo[SVGA_FIFO_NEXT_CMD];
        assert!(next_cmd % 4 == 0);

        fifo[next_cmd as usize / 4] = value;
        next_cmd += 4;

        if next_cmd >= fifo[SVGA_FIFO_MAX] {
            next_cmd = fifo[SVGA_FIFO_MIN];
        }

        fifo[SVGA_FIFO_NEXT_CMD] = next_cmd;
    }
}

pub fn vmware_svga2_update(x: u32, y: u32, width: u32, height: u32) {
    vmware_svga2_fifo_write(SVGA_CMD_UPDATE);
    vmware_svga2_fifo_write(x);
    vmware_svga2_fifo_write(y);
    vmware_svga2_fifo_write(width);
    vmware_svga2_fifo_write(height);
}

pub fn initiate_vmware_svga2(device: &PciDevice) {
    if !vmware_svga2_detect(device) { return; }

    debugf("[pci::svga-II] VMWare SVGA-II graphics card detected!\n");

    assert!(!unsafe { VMWareSvga2.exists });

    let details = get_general_device(device);
    unsafe { VMWareSvga2.base = details.bar[0] & !0x3; }

    // Enable PCI command bits
    let mut command_status = combine_word(device.status, device.command);
    command_status |= 0x7;
    config_write_dword(device.bus, device.slot, device.function, PCI_COMMAND, command_status);

    // set latest specification ID
    vmware_svga2_write(SVGA_REG_ID, 0x9000_0002);
    assert!(vmware_svga2_read(SVGA_REG_ID) == 0x9000_0002);

    // pretend Linux guest
    vmware_svga2_write(23, SVGA_GUEST_OS_LINUX);

    unsafe {
        VMWareSvga2.fifo = (bootloader().hhdm_offset + vmware_svga2_read(SVGA_REG_FIFO_START) as usize) as *mut u32;
        let fifo = &mut *VMWareSvga2.fifo;
        fifo[SVGA_FIFO_MIN] = 293 * 4;
        fifo[SVGA_FIFO_MAX] = vmware_svga2_read(SVGA_REG_FIFO_SIZE);
        fifo[SVGA_FIFO_NEXT_CMD] = 293 * 4;
        fifo[SVGA_FIFO_STOP] = 293 * 4;
    }

    debugf!("Max: {}x{}", vmware_svga2_read(SVGA_REG_MAX_WIDTH), vmware_svga2_read(SVGA_REG_MAX_HEIGHT));
    debugf!("VRAM size: {} bytes", vmware_svga2_read(SVGA_REG_VRAM_SIZE));

    // Set resolution 1920x1080 32bpp
    vmware_svga2_set_mode(1920, 1080, 32);
    vmware_svga2_write(SVGA_REG_CONFIG_DONE, 1);
    vmware_svga2_write(SVGA_REG_ENABLE, 1);
    assert!(vmware_svga2_read(SVGA_REG_ENABLE) == 1);
    assert!(vmware_svga2_read(SVGA_REG_CONFIG_DONE) == 1);

    vmware_svga2_sync();

    draw_rect(0, 0, fb().width, fb().height, 255, 255, 0);
    vmware_svga2_update(0, 0, fb().width, fb().height);
    unsafe { VMWareSvga2.exists = true; }
}
