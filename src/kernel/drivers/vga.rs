#![no_std]

use core::ptr::null_mut;

//
// ===== Limine definitions =====
//

#[repr(C)]
pub struct LimineFramebufferRequest {
    pub id: u64,
    pub revision: u64,
    pub response: *mut LimineFramebufferResponse,
}

#[repr(C)]
pub struct LimineFramebufferResponse {
    pub framebuffer_count: u64,
    pub framebuffers: *mut *mut LimineFramebuffer,
}

#[repr(C)]
pub struct LimineFramebuffer {
    pub address: *mut u8,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

//
// ===== Extern kernel symbols =====
//

extern "C" {
    static mut fb: Framebuffer;
    static bootloader: BootloaderInfo;

    fn debugf(fmt: *const u8, ...) -> i32;
}

//
// ===== Kernel structs =====
//

#[repr(C)]
pub struct Framebuffer {
    pub virt: *mut u8,
    pub phys: usize,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u16,

    pub red_shift: u8,
    pub red_size: u8,

    pub green_shift: u8,
    pub green_size: u8,

    pub blue_shift: u8,
    pub blue_size: u8,
}

#[repr(C)]
pub struct BootloaderInfo {
    pub hhdmOffset: usize,
}

//
// ===== Limine constants =====
//

const LIMINE_FRAMEBUFFER_REQUEST: u64 = 0x9d5827dcd881dd75;

//
// ===== Static request =====
//

#[no_mangle]
#[used]
static mut LIMINE_FB_REQ: LimineFramebufferRequest = LimineFramebufferRequest {
    id: LIMINE_FRAMEBUFFER_REQUEST,
    revision: 0,
    response: null_mut(),
};

//
// ===== VGA / framebuffer init =====
//

#[no_mangle]
pub extern "C" fn initiateVGA() {
    unsafe {
        let response = LIMINE_FB_REQ.response;
        if response.is_null() {
            return;
        }

        let fb_array = (*response).framebuffers;
        if fb_array.is_null() {
            return;
        }

        let framebuffer = *fb_array; // framebuffers[0]

        fb.virt = (*framebuffer).address;
        fb.phys = fb.virt as usize - bootloader.hhdmOffset;

        fb.width = (*framebuffer).width;
        fb.height = (*framebuffer).height;
        fb.pitch = (*framebuffer).pitch;
        fb.bpp = (*framebuffer).bpp;

        fb.red_shift = (*framebuffer).red_mask_shift;
        fb.red_size = (*framebuffer).red_mask_size;

        fb.green_shift = (*framebuffer).green_mask_shift;
        fb.green_size = (*framebuffer).green_mask_size;

        fb.blue_shift = (*framebuffer).blue_mask_shift;
        fb.blue_size = (*framebuffer).blue_mask_size;

        debugf(
            b"[graphics] Resolution fixed: fb{%lx} dim(xy){%dx%d} bpp{%d}\n\0"
                .as_ptr(),
            fb.virt,
            fb.width,
            fb.height,
            fb.bpp,
        );
    }
}
