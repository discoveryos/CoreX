#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::null_mut;

extern crate libc;

// --------------------- Linux framebuffer structs ---------------------
const FBIOGET_VSCREENINFO: u64 = 0x4600;

#[repr(C)]
#[derive(Default)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[repr(C)]
#[derive(Default)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

// --------------------- Hardcoded framebuffer image size ---------------------
const WIDTH: usize = 800;
const HEIGHT: usize = 600;

// --------------------- File reading ---------------------
unsafe fn read_full_file(filename: *const u8) -> *mut u8 {
    use libc::{fopen, fseek, ftell, fread, fclose, malloc, SEEK_END, SEEK_SET};

    let mode = b"rb\0".as_ptr();
    let f = fopen(filename as *const i8, mode as *const i8);
    if f.is_null() { return null_mut(); }

    fseek(f, 0, SEEK_END);
    let length = ftell(f) as usize;
    fseek(f, 0, SEEK_SET);

    let buffer = malloc(length) as *mut u8;
    if buffer.is_null() { fclose(f); return null_mut(); }

    fread(buffer as *mut core::ffi::c_void, 1, length, f);
    fclose(f);
    buffer
}

// --------------------- Entry point ---------------------
#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc != 2 {
        unsafe {
            let msg = b"Wrong parameters! Usage:\n\t./drawimg ./img.cavpng\n\0";
            libc::printf(msg.as_ptr() as *const i8);
        }
        unsafe { libc::exit(1) };
    }

    let filename = unsafe { *argv.offset(1) };

    unsafe {
        // Open framebuffer
        let fb_fd = libc::open(b"/dev/fb0\0".as_ptr() as *const i8, libc::O_RDWR);
        if fb_fd < 0 {
            let msg = b"Couldn't open framebuffer! /dev/fb0\n\0";
            libc::printf(msg.as_ptr() as *const i8);
            libc::exit(1);
        }

        // Get framebuffer info
        let mut fb_info: FbVarScreeninfo = core::mem::zeroed();
        if libc::ioctl(fb_fd, FBIOGET_VSCREENINFO, &mut fb_info) < 0 {
            let msg = b"Couldn't get framebuffer info!\n\0";
            libc::printf(msg.as_ptr() as *const i8);
            libc::exit(1);
        }

        // mmap framebuffer memory
        let fb_size = (fb_info.xres * fb_info.yres * 4) as usize;
        let fb_region = libc::mmap(
            null_mut(),
            fb_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fb_fd,
            0,
        ) as *mut u8;

        if fb_region == libc::MAP_FAILED as *mut u8 {
            let msg = b"Couldn't mmap framebuffer!\n\0";
            libc::printf(msg.as_ptr() as *const i8);
            libc::exit(1);
        }

        // Read image file
        let buff = read_full_file(filename);
        if buff.is_null() {
            let msg = b"Couldn't read file!\n\0";
            libc::printf(msg.as_ptr() as *const i8);
            libc::exit(1);
        }

        // Clear terminal
        libc::printf(b"\x1B[H\x1B[J\0".as_ptr() as *const i8);

        // Vertical spacing
        let amnt = fb_info.yres / 16;
        for _ in 0..amnt { libc::printf(b"\n\0".as_ptr() as *const i8); }

        // Draw pixels
        for i in 0..(WIDTH * HEIGHT) {
            let buffer_pos = i * 3;
            let r = *buff.add(buffer_pos);
            let g = *buff.add(buffer_pos + 1);
            let b = *buff.add(buffer_pos + 2);

            let x = i % WIDTH;
            let y = i / WIDTH;

            let offset = (x + y * fb_info.xres as usize) * 4;

            *fb_region.add(offset) = b;
            *fb_region.add(offset + 1) = g;
            *fb_region.add(offset + 2) = r;
            *fb_region.add(offset + 3) = 0;
        }

        libc::free(buff as *mut core::ffi::c_void);
    }

    0
}

// --------------------- _start for no-std ---------------------
#[no_mangle]
pub extern "C" fn _start_c(rsp: u64) -> ! {
    let argc = unsafe { *(rsp as *const i32) };
    let argv = unsafe { (rsp + 8) as *const *const u8 };
    main(argc, argv);
    unsafe { libc::exit(0) };
    loop {}
}

// --------------------- Panic handler ---------------------
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
