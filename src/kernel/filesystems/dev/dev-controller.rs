#![no_std]
#![no_main]

use core::ptr;
use core::mem::MaybeUninit;
use crate::task::*;
use crate::fb::*;
use crate::vfs::*;
use crate::util::*;

pub struct Dev;

pub static mut ROOT_DEV: Fakefs = Fakefs {
    root_file: MaybeUninit::uninit(),
};

/// Very primitive random read
pub fn random_read(fd: &OpenFile, out: &mut [u8]) -> usize {
    let len = out.len();
    let div4 = len / 4;
    let rem = len % 4;

    let out_u32 = unsafe { core::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u32, div4) };
    for i in 0..div4 {
        out_u32[i] = rand();
    }

    for i in 0..rem {
        out[len - 1 - i] = (rand() & 0xFF) as u8;
    }

    len
}

pub static HANDLE_RANDOM: VfsHandlers = VfsHandlers {
    read: Some(random_read),
    stat: Some(fakefs_fstat),
    ..VfsHandlers::default()
};

/// Open a /dev/tty for the current task
pub fn dev_tty_open(
    _filename: &str,
    _flags: u32,
    _mode: u32,
    fd: &mut OpenFile,
) -> Result<(), i32> {
    let task = current_task();

    if task.ctrl_pty == -1 {
        return Err(ENXIO);
    }

    let mut path = [0u8; 128];
    let _ = snprintf(&mut path, "/pts/{}", task.ctrl_pty);

    fd.handlers = &HANDLE_PTS;
    pts_open(&path, fd)?;
    Ok(())
}

pub static HANDLE_TTY: VfsHandlers = VfsHandlers {
    open: Some(dev_tty_open),
    ..VfsHandlers::default()
};

/// Setup /dev filesystem
pub fn dev_setup() {
    unsafe {
        let root_file = &mut ROOT_DEV.root_file;
        fakefs_add_file(root_file, "stdin", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_STDIO);
        fakefs_add_file(root_file, "stdout", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_STDIO);
        fakefs_add_file(root_file, "stderr", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_STDIO);
        fakefs_add_file(root_file, "tty", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_TTY);
        fakefs_add_file(root_file, "fb0", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_FB0);
        fakefs_add_file(root_file, "null", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_NULL);
        fakefs_add_file(root_file, "random", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_RANDOM);
        fakefs_add_file(root_file, "urandom", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_RANDOM);

        initiate_pty_interface();
        fakefs_add_file(root_file, "ptmx", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_PTMX);

        let pts = fakefs_add_file(root_file, "pts", 0, S_IFDIR | S_IRUSR | S_IWUSR, &FAKEFS_ROOT_HANDLERS);
        fakefs_add_file(&pts, "*", 0, S_IFCHR | S_IRUSR | S_IWUSR, &HANDLE_PTS);

        INPUT_FAKE_DIR = fakefs_add_file(root_file, "input", 0, S_IFDIR | S_IRUSR | S_IWUSR, &FAKEFS_ROOT_HANDLERS);
    }
}

/// Mount /dev
pub fn dev_mount(mount: &mut MountPoint) -> bool {
    mount.handlers = Some(&FAKEFS_HANDLERS);
    mount.stat = Some(fakefs_stat);
    mount.lstat = Some(fakefs_lstat);

    mount.fs_info = Box::into_raw(Box::new(FakefsOverlay::default())) as *mut _;

    let dev_overlay = unsafe { &mut *(mount.fs_info as *mut FakefsOverlay) };
    dev_overlay.fakefs = unsafe { &mut ROOT_DEV };

    unsafe {
        if ROOT_DEV.root_file.as_ptr().is_null() {
            fakefs_setup_root(&mut ROOT_DEV.root_file);
            dev_setup();
        }
    }

    true
}
