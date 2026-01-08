use core::fmt::Write;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::cell::RefCell;
use spin::Mutex;

use crate::fb::*;
use crate::syscalls::*;
use crate::util::*;
use crate::pci::*;

/// The root of the sys filesystem
pub struct FakeFs {
    pub root: RefCell<Vec<Box<FakeFsFile>>>,
}

/// Represents a file in the fake filesystem
pub struct FakeFsFile {
    pub name: String,
    pub kind: FsKind,
    pub handlers: Option<FsHandlers>,
    pub extra: Option<Box<dyn AnyFsExtra>>,
    pub children: RefCell<Vec<Box<FakeFsFile>>>,
}

pub enum FsKind {
    File,
    Dir,
    Symlink(String),
}

/// Trait for extra data attached to a file
pub trait AnyFsExtra {}

/// PCI configuration info
pub struct PciConf {
    pub bus: u16,
    pub slot: u8,
    pub function: u8,
}
impl AnyFsExtra for PciConf {}

/// Handlers for a file
pub struct FsHandlers {
    pub read: Option<fn(&mut OpenFile, &mut [u8]) -> usize>,
    pub write: Option<fn(&mut OpenFile, &[u8]) -> usize>,
    pub stat: Option<fn(&FakeFsFile) -> FsStat>,
    pub seek: Option<fn(&mut OpenFile, usize)>,
    // Additional handlers can be added here
}

/// Open file state
pub struct OpenFile {
    pub pointer: usize,
    pub file: Box<FakeFsFile>,
}

/// PCI config file read handler
fn pci_config_read(fd: &mut OpenFile, out: &mut [u8]) -> usize {
    let conf = fd.file.extra.as_ref().unwrap()
        .downcast_ref::<PciConf>().unwrap();

    if fd.pointer >= 4096 { return 0; }
    let to_copy = core::cmp::min(4096 - fd.pointer, out.len());

    for i in 0..to_copy {
        let word = unsafe { config_read_word(conf.bus, conf.slot, conf.function, fd.pointer as u8) };
        out[i] = export_byte(word, true);
        fd.pointer += 1;
    }
    to_copy
}

/// Console write handler
fn cavos_console_write(fd: &mut OpenFile, buff: &[u8]) -> usize {
    if let Some(&b) = buff.last() {
        if b == b'd' { unsafe { CONSOLE_DISABLED = true; } }
        else if b == b'e' { unsafe { CONSOLE_DISABLED = false; } }
    }
    buff.len()
}

/// Setup PCI devices under `/sys/bus/pci/devices`
fn sys_setup_pci(devices_dir: &mut FakeFsFile) {
    for bus in 0..PCI_MAX_BUSES {
        for slot in 0..PCI_MAX_DEVICES {
            for function in 0..PCI_MAX_FUNCTIONS {
                if !filter_device(bus, slot, function) { continue; }

                let device = get_device(bus, slot, function);
                let gen = get_general_device(&device);

                let dirname = format!("0000:{:02}:{:02}.{}", bus, slot, function);
                let mut dir = FakeFsFile {
                    name: dirname,
                    kind: FsKind::Dir,
                    handlers: Some(FsHandlers {
                        read: None, write: None, stat: None, seek: None
                    }),
                    extra: None,
                    children: RefCell::new(vec![]),
                };

                // config
                let conf = PciConf { bus, slot, function };
                let config_file = FakeFsFile {
                    name: "config".into(),
                    kind: FsKind::File,
                    handlers: Some(FsHandlers {
                        read: Some(pci_config_read),
                        write: None, stat: None, seek: None
                    }),
                    extra: Some(Box::new(conf)),
                    children: RefCell::new(vec![]),
                };
                dir.children.borrow_mut().push(Box::new(config_file));

                // vendor
                let vendor_file = FakeFsFile {
                    name: "vendor".into(),
                    kind: FsKind::File,
                    handlers: None,
                    extra: Some(Box::new(format!("0x{:04x}\n", device.vendor_id))),
                    children: RefCell::new(vec![]),
                };
                dir.children.borrow_mut().push(Box::new(vendor_file));

                // device
                let device_file = FakeFsFile {
                    name: "device".into(),
                    kind: FsKind::File,
                    handlers: None,
                    extra: Some(Box::new(format!("0x{:04x}\n", device.device_id))),
                    children: RefCell::new(vec![]),
                };
                dir.children.borrow_mut().push(Box::new(device_file));

                // irq
                let irq_file = FakeFsFile {
                    name: "irq".into(),
                    kind: FsKind::File,
                    handlers: None,
                    extra: Some(Box::new(format!("{}\n", gen.interrupt_line))),
                    children: RefCell::new(vec![]),
                };
                dir.children.borrow_mut().push(Box::new(irq_file));

                // revision
                let revision_file = FakeFsFile {
                    name: "revision".into(),
                    kind: FsKind::File,
                    handlers: None,
                    extra: Some(Box::new(format!("0x{:02x}\n", device.revision))),
                    children: RefCell::new(vec![]),
                };
                dir.children.borrow_mut().push(Box::new(revision_file));

                // class
                let class_code: u32 = ((config_read_word(bus, slot, function, PCI_SUBCLASS) as u32) << 8)
                    | ((device.prog_if as u32) << 8);
                let class_file = FakeFsFile {
                    name: "class".into(),
                    kind: FsKind::File,
                    handlers: None,
                    extra: Some(Box::new(format!("0x{:x}\n", class_code))),
                    children: RefCell::new(vec![]),
                };
                dir.children.borrow_mut().push(Box::new(class_file));

                devices_dir.children.borrow_mut().push(Box::new(dir));
            }
        }
    }
}

/// Initialize `/sys` pseudo-filesystem
pub fn sys_setup(root: &mut FakeFs) {
    let root_file = FakeFsFile {
        name: "/".into(),
        kind: FsKind::Dir,
        handlers: Some(FsHandlers { read: None, write: None, stat: None, seek: None }),
        extra: None,
        children: RefCell::new(vec![]),
    };

    root.root.borrow_mut().push(Box::new(root_file));

    // cavosConsole
    let cavos_console = FakeFsFile {
        name: "cavosConsole".into(),
        kind: FsKind::File,
        handlers: Some(FsHandlers { read: None, write: Some(cavos_console_write), stat: None, seek: None }),
        extra: None,
        children: RefCell::new(vec![]),
    };
    root.root.borrow_mut().push(Box::new(cavos_console));

    // bus/pci/devices
    let mut bus = FakeFsFile { name: "bus".into(), kind: FsKind::Dir, handlers: None, extra: None, children: RefCell::new(vec![]) };
    let mut pci = FakeFsFile { name: "pci".into(), kind: FsKind::Dir, handlers: None, extra: None, children: RefCell::new(vec![]) };
    let mut devices = FakeFsFile { name: "devices".into(), kind: FsKind::Dir, handlers: None, extra: None, children: RefCell::new(vec![]) };

    sys_setup_pci(&mut devices);

    pci.children.borrow_mut().push(Box::new(devices));
    bus.children.borrow_mut().push(Box::new(pci));
    root.root.borrow_mut().push(Box::new(bus));
}
