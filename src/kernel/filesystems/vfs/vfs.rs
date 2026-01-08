use alloc::{boxed::Box, string::String, vec::Vec};
use core::cell::RefCell;
use spin::Mutex;
use alloc::rc::Rc;
use hashbrown::HashMap;

/// --- VFS layer types ---
pub type FileId = usize;

#[derive(Clone)]
pub struct OpenFile {
    pub id: FileId,
    pub pointer: usize,
    pub mode: u32,
    pub flags: u32,
    pub close_on_exec: bool,
    pub close_flags: VfsCloseFlag,
    pub handlers: Option<Rc<VfsHandlers>>,
    pub mount_point: Option<Rc<MountPoint>>,
    pub lock_operations: Mutex<()>,
    pub tmp1: u32,
}

impl OpenFile {
    pub fn new(id: FileId) -> Self {
        OpenFile {
            id,
            pointer: 0,
            mode: 0,
            flags: 0,
            close_on_exec: false,
            close_flags: VfsCloseFlag::empty(),
            handlers: None,
            mount_point: None,
            lock_operations: Mutex::new(()),
            tmp1: 0,
        }
    }

    pub fn copy_from(&mut self, other: &OpenFile) {
        self.pointer = other.pointer;
        self.mode = other.mode;
        self.flags = other.flags;
        self.close_on_exec = other.close_on_exec;
        self.close_flags = other.close_flags;
        self.handlers = other.handlers.clone();
        self.mount_point = other.mount_point.clone();
        self.tmp1 = other.tmp1;
    }
}

bitflags::bitflags! {
    pub struct VfsCloseFlag: u32 {
        const RetainId = 0x1;
    }
}

pub struct VfsHandlers {
    pub open: Option<fn(&str, u32, u32, &mut OpenFile) -> Result<(), VfsError>>,
    pub read: Option<fn(&OpenFile, &mut [u8]) -> usize>,
    pub write: Option<fn(&OpenFile, &[u8]) -> usize>,
    pub seek: Option<fn(&OpenFile, usize, isize, SeekWhence) -> usize>,
    pub close: Option<fn(&OpenFile) -> bool>,
    pub duplicate: Option<fn(&OpenFile, &mut OpenFile) -> bool>,
    pub internal_poll: bool,
    pub recv_from: Option<fn(&OpenFile, &mut [u8]) -> usize>,
    pub send_to: Option<fn(&OpenFile, &[u8]) -> usize>,
}

pub enum SeekWhence {
    Set,
    Cur,
    End,
}

pub struct TaskInfoFiles {
    pub wlock_files: Mutex<()>,
    pub first_file: HashMap<FileId, Box<OpenFile>>,
    pub rlimit_fds_soft: usize,
    pub fd_bitmap: Vec<bool>,
}

pub struct Task {
    pub info_files: TaskInfoFiles,
    pub info_fs: InfoFs,
    pub execname: Option<String>,
}

pub struct InfoFs {
    pub cwd: String,
    pub lock_fs: Mutex<()>,
}

/// --- Mountpoint abstraction ---
pub struct MountPoint {
    pub handlers: Rc<VfsHandlers>,
    pub readlink: Option<fn(&MountPoint, &str, &mut [u8]) -> usize>,
    pub mkdir: Option<fn(&MountPoint, &str, u32) -> usize>,
    pub delete: Option<fn(&MountPoint, &str, bool) -> usize>,
    pub link: Option<fn(&MountPoint, &str, &str) -> usize>,
}

/// --- Simple FakeFS layer ---
pub struct FakeFsFile {
    pub name: String,
    pub content: Vec<u8>,
    pub children: Vec<Rc<RefCell<FakeFsFile>>>,
    pub handlers: Option<Rc<VfsHandlers>>,
    pub is_dir: bool,
}

impl FakeFsFile {
    pub fn new(name: &str, is_dir: bool) -> Self {
        FakeFsFile {
            name: name.to_string(),
            content: Vec::new(),
            children: Vec::new(),
            handlers: None,
            is_dir,
        }
    }
}

/// Root system pseudo-filesystem
pub struct FakeFs {
    pub root: Rc<RefCell<FakeFsFile>>,
}

impl FakeFs {
    pub fn new() -> Self {
        FakeFs {
            root: Rc::new(RefCell::new(FakeFsFile::new("/", true))),
        }
    }

    pub fn add_file(
        &self,
        parent: &Rc<RefCell<FakeFsFile>>,
        name: &str,
        is_dir: bool,
        handlers: Option<Rc<VfsHandlers>>,
        content: Option<Vec<u8>>,
    ) -> Rc<RefCell<FakeFsFile>> {
        let mut file = FakeFsFile::new(name, is_dir);
        file.handlers = handlers;
        if let Some(data) = content {
            file.content = data;
        }
        let rc = Rc::new(RefCell::new(file));
        parent.borrow_mut().children.push(rc.clone());
        rc
    }
}

/// --- PCI /sys setup ---
pub struct PciConf {
    pub bus: u16,
    pub slot: u8,
    pub function: u8,
}

pub fn sys_setup(fakefs: &FakeFs) {
    let root = fakefs.root.clone();
    let bus = fakefs.add_file(&root, "bus", true, None, None);
    let pci = fakefs.add_file(&bus, "pci", true, None, None);
    let devices = fakefs.add_file(&pci, "devices", true, None, None);

    // Example: simulate PCI device 00:02.0
    let dirname = "0000:00:02.0";
    let device_dir = fakefs.add_file(&devices, dirname, true, None, None);

    // config file
    let pci_conf = PciConf {
        bus: 0,
        slot: 2,
        function: 0,
    };
    let config_handlers = Rc::new(VfsHandlers {
        read: Some(|file, out| {
            let word: u16 = 0x1234;
            out[0] = (word & 0xFF) as u8;
            1
        }),
        write: None,
        open: None,
        seek: Some(|file, target, _, _| {
            file.pointer = target;
            0
        }),
        close: None,
        duplicate: None,
        internal_poll: false,
        recv_from: None,
        send_to: None,
    });
    fakefs.add_file(&device_dir, "config", false, Some(config_handlers), None);

    // vendor file
    fakefs.add_file(
        &device_dir,
        "vendor",
        false,
        None,
        Some(b"0x8086\n".to_vec()),
    );
    // irq file
    fakefs.add_file(
        &device_dir,
        "irq",
        false,
        None,
        Some(b"16\n".to_vec()),
    );
    // revision file
    fakefs.add_file(
        &device_dir,
        "revision",
        false,
        None,
        Some(b"0x0a\n".to_vec()),
    );
    // device file
    fakefs.add_file(
        &device_dir,
        "device",
        false,
        None,
        Some(b"0x1234\n".to_vec()),
    );
    // class file
    fakefs.add_file(
        &device_dir,
        "class",
        false,
        None,
        Some(b"0x030000\n".to_vec()),
    );
}
