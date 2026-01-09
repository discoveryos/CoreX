// System framebuffer manager
// Copyright (C) 2025 kevin dan mathew
// Rust translation

#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ffi::c_int;
use core::ptr;

// --------------------------------
// External types & constants
// --------------------------------

#[repr(C)]
pub struct Framebuffer {
    pub virt: *mut u8,
    pub phys: usize,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,

    pub red_shift: u32,
    pub red_size: u32,
    pub green_shift: u32,
    pub green_size: u32,
    pub blue_shift: u32,
    pub blue_size: u32,

    pub bpp: u32,
}

#[repr(C)]
pub struct OpenFile {
    _private: [u8; 0],
}

#[repr(C)]
pub struct stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_blksize: u64,
    pub st_size: u64,
    pub st_blocks: u64,
    pub st_atime: u64,
    pub st_mtime: u64,
    pub st_ctime: u64,
}

#[repr(C)]
pub struct fb_bitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

#[repr(C)]
pub struct fb_fix_screeninfo {
    pub id: [u8; 16],
    pub smem_start: usize,
    pub smem_len: usize,
    pub type_: u32,
    pub type_aux: u32,
    pub visual: u32,
    pub xpanstep: u16,
    pub ypanstep: u16,
    pub ywrapstep: u16,
    pub line_length: u32,
    pub mmio_start: usize,
    pub mmio_len: usize,
    pub capabilities: u32,
}

#[repr(C)]
pub struct fb_var_screeninfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,

    pub red: fb_bitfield,
    pub green: fb_bitfield,
    pub blue: fb_bitfield,
    pub transp: fb_bitfield,

    pub bits_per_pixel: u32,
    pub grayscale: u32,
    pub nonstd: u32,
    pub activate: u32,
    pub height: u32,
    pub width: u32,
}

#[repr(C)]
pub struct VfsHandlers {
    pub open: Option<extern "C" fn()>,
    pub close: Option<extern "C" fn()>,
    pub read: extern "C" fn(*mut OpenFile, usize, *mut u8) -> usize,
    pub write: extern "C" fn(*mut OpenFile, usize, *const u8) -> usize,
    pub ioctl: extern "C" fn(*mut OpenFile, u64, *mut core::ffi::c_void) -> usize,
    pub mmap: extern "C" fn(
        usize,
        usize,
        c_int,
        c_int,
        *mut OpenFile,
        usize,
    ) -> usize,
    pub stat: extern "C" fn(*mut OpenFile, *mut stat) -> usize,
    pub duplicate: Option<extern "C" fn()>,
    pub getdents64: Option<extern "C" fn()>,
}

// --------------------------------
// External kernel symbols
// --------------------------------

extern "C" {
    fn memcpy(dest: *mut u8, src: *const u8, n: usize);
    fn debugf(fmt: *const u8, ...);
    fn rand() -> u64;

    fn VirtualMap(virt: usize, phys: usize, flags: usize);

    fn DivRoundUp(a: usize, b: usize) -> usize;
}

// --------------------------------
// Constants
// --------------------------------

const FB_TYPE_PACKED_PIXELS: u32 = 0;
const FB_VISUAL_TRUECOLOR: u32 = 2;

const FBIOGET_FSCREENINFO: u64 = 0x4602;
const FBIOGET_VSCREENINFO: u64 = 0x4600;
const FBIOPUT_VSCREENINFO: u64 = 0x4601;

const ENOTTY: usize = 25;
const PAGE_SIZE: usize = 4096;

const PF_RW: usize = 1 << 1;
const PF_USER: usize = 1 << 2;
const PF_CACHE_WC: usize = 1 << 7;

const S_IFCHR: u32 = 0o020000;
const S_IRUSR: u32 = 0o400;
const S_IWUSR: u32 = 0o200;

#[inline]
fn ERR(e: usize) -> usize {
    (!e + 1)
}

// --------------------------------
// Global framebuffer
// --------------------------------

#[no_mangle]
pub static mut fb: Framebuffer = unsafe { core::mem::zeroed() };

// --------------------------------
// Drawing
// --------------------------------

#[no_mangle]
pub unsafe extern "C" fn drawRect(
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    r: c_int,
    g: c_int,
    b: c_int,
) {
    let mut offset = ((x as usize + y as usize * fb.width) * 4) as usize;

    for _ in 0..h {
        for j in 0..w {
            let base = offset + (j as usize) * 4;
            *fb.virt.add(base) = b as u8;
            *fb.virt.add(base + 1) = g as u8;
            *fb.virt.add(base + 2) = r as u8;
            *fb.virt.add(base + 3) = 0;
        }
        offset += fb.pitch;
    }
}

// --------------------------------
// Userspace handlers
// --------------------------------

#[no_mangle]
pub extern "C" fn fbUserIllegal(
    _fd: *mut OpenFile,
    _len: usize,
    _buf: *mut u8,
) -> usize {
    unsafe {
        debugf(b"[io::fb] Tried to do anything but an mmap/ioctl!\n\0".as_ptr());
    }
    usize::MAX
}

#[no_mangle]
pub unsafe extern "C" fn fbUserIoctl(
    _fd: *mut OpenFile,
    request: u64,
    arg: *mut core::ffi::c_void,
) -> usize {
    match request {
        FBIOGET_FSCREENINFO => {
            let fbtarg = arg as *mut fb_fix_screeninfo;

            memcpy(
                (*fbtarg).id.as_mut_ptr(),
                b"BIOS\0".as_ptr(),
                5,
            );

            (*fbtarg).smem_start = fb.phys;
            (*fbtarg).smem_len = fb.width * fb.height * 4;
            (*fbtarg).type_ = FB_TYPE_PACKED_PIXELS;
            (*fbtarg).type_aux = 0;
            (*fbtarg).visual = FB_VISUAL_TRUECOLOR;
            (*fbtarg).xpanstep = 0;
            (*fbtarg).ypanstep = 0;
            (*fbtarg).ywrapstep = 0;
            (*fbtarg).line_length = (fb.width * 4) as u32;
            (*fbtarg).mmio_start = fb.phys;
            (*fbtarg).mmio_len = fb.width * fb.height * 4;
            (*fbtarg).capabilities = 0;
            0
        }

        FBIOPUT_VSCREENINFO => 0,

        0x4605 => 0, // FBIOPUTCMAP (ignored)

        FBIOGET_VSCREENINFO => {
            let fbtarg = arg as *mut fb_var_screeninfo;

            (*fbtarg).xres = fb.width as u32;
            (*fbtarg).yres = fb.height as u32;
            (*fbtarg).xres_virtual = fb.width as u32;
            (*fbtarg).yres_virtual = fb.height as u32;

            (*fbtarg).red = fb_bitfield {
                offset: fb.red_shift,
                length: fb.red_size,
                msb_right: 1,
            };
            (*fbtarg).green = fb_bitfield {
                offset: fb.green_shift,
                length: fb.green_size,
                msb_right: 1,
            };
            (*fbtarg).blue = fb_bitfield {
                offset: fb.blue_shift,
                length: fb.blue_size,
                msb_right: 1,
            };
            (*fbtarg).transp = fb_bitfield {
                offset: 24,
                length: 8,
                msb_right: 1,
            };

            (*fbtarg).bits_per_pixel = fb.bpp;
            (*fbtarg).grayscale = 0;
            (*fbtarg).nonstd = 0;
            (*fbtarg).activate = 0;
            (*fbtarg).height = (fb.height / 4) as u32;
            (*fbtarg).width = (fb.width / 4) as u32;
            0
        }

        _ => ERR(ENOTTY),
    }
}

#[no_mangle]
pub unsafe extern "C" fn fbUserMmap(
    _addr: usize,
    mut length: usize,
    _prot: c_int,
    _flags: c_int,
    _fd: *mut OpenFile,
    _pgoffset: usize,
) -> usize {
    if length == 0 {
        length = fb.width * fb.height * 4;
    }

    let pages = DivRoundUp(length, PAGE_SIZE);
    let phys = fb.phys;

    for i in 0..pages {
        VirtualMap(
            0x1500_0000_0000 + i * PAGE_SIZE,
            phys + i * PAGE_SIZE,
            PF_RW | PF_USER | PF_CACHE_WC,
        );
    }

    0x1500_0000_0000
}

#[no_mangle]
pub unsafe extern "C" fn fbUserStat(_fd: *mut OpenFile, target: *mut stat) -> usize {
    (*target).st_dev = 70;
    (*target).st_ino = rand();
    (*target).st_mode = S_IFCHR | S_IRUSR | S_IWUSR;
    (*target).st_nlink = 1;
    (*target).st_uid = 0;
    (*target).st_gid = 0;
    (*target).st_rdev = 0;
    (*target).st_blksize = 0x1000;
    (*target).st_size = 0;
    (*target).st_blocks = DivRoundUp((*target).st_size as usize, 512) as u64;
    (*target).st_atime = 69;
    (*target).st_mtime = 69;
    (*target).st_ctime = 69;
    0
}

// --------------------------------
// VFS registration
// --------------------------------

#[no_mangle]
pub static fb0: VfsHandlers = VfsHandlers {
    open: None,
    close: None,
    read: fbUserIllegal,
    write: fbUserIllegal,
    ioctl: fbUserIoctl,
    mmap: fbUserMmap,
    stat: fbUserStat,
    duplicate: None,
    getdents64: None,
};
