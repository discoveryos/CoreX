#![allow(dead_code)]

use core::sync::atomic::{
    AtomicU8, AtomicU16, AtomicU32, AtomicU64,
    Ordering,
};

//
// ANSI colors
//
pub const ANSI_RESET: &str  = "\x1b[0m";
pub const ANSI_BLACK: &str  = "\x1b[0;30m";
pub const ANSI_RED: &str    = "\x1b[0;31m";
pub const ANSI_GREEN: &str  = "\x1b[0;32m";
pub const ANSI_YELLOW: &str = "\x1b[0;33m";
pub const ANSI_BLUE: &str   = "\x1b[0;34m";
pub const ANSI_PURPLE: &str = "\x1b[0;35m";
pub const ANSI_CYAN: &str   = "\x1b[0;36m";
pub const ANSI_WHITE: &str  = "\x1b[0;37m";

//
// Linux errno strings
//
pub const LINUX_ERRNO: [&str; 37] = [
    "EPERM", "ENOENT", "ESRCH", "EINTR", "EIO", "ENXIO",
    "E2BIG", "ENOEXEC", "EBADF", "ECHILD", "EAGAIN", "ENOMEM",
    "EACCES", "EFAULT", "ENOTBLK", "EBUSY", "EEXIST", "EXDEV",
    "ENODEV", "ENOTDIR", "EISDIR", "EINVAL", "ENFILE", "EMFILE",
    "ENOTTY", "ETXTBSY", "EFBIG", "ENOSPC", "ESPIPE", "EROFS",
    "EMLINK", "EPIPE", "EDOM", "ERANGE", "EDEADLK", "ENAMETOOLONG",
    "ENOLCK",
];

//
// Signal strings
//
pub const SIGNALS: [&str; 34] = [
    "ZERO?", "SIGHUP", "SIGINT", "SIGQUIT", "SIGILL", "SIGTRAP",
    "SIGABRT", "SIGBUS", "SIGFPE", "SIGKILL", "SIGUSR1", "SIGSEGV",
    "SIGUSR2", "SIGPIPE", "SIGALRM", "SIGTERM", "SIGSTKFLT", "SIGCHLD",
    "SIGCONT", "SIGSTOP", "SIGTSTP", "SIGTTIN", "SIGTTOU", "SIGURG",
    "SIGXCPU", "SIGXFSZ", "SIGVTALRM", "SIGPROF", "SIGWINCH", "SIGIO",
    "SIGPWR", "SIGSYS", "SIGUNUSED",
];

pub const SIG_STR_DEFAULT: &str = "SIGRTn";

pub fn signal_str(signum: i32) -> &'static str {
    if signum >= 0 && signum < 32 {
        SIGNALS[signum as usize]
    } else {
        SIG_STR_DEFAULT
    }
}

//
// Memory functions
// (Unsafe by nature, mirrors C semantics)
//
pub unsafe fn memset(dst: *mut u8, val: u8, len: usize) {
    core::ptr::write_bytes(dst, val, len);
}

pub unsafe fn memcpy(dst: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    core::ptr::copy_nonoverlapping(src, dst, len);
    dst
}

pub unsafe fn memmove(dst: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    core::ptr::copy(src, dst, len);
    dst
}

pub fn memcmp(a: *const u8, b: *const u8, len: usize) -> i32 {
    unsafe {
        for i in 0..len {
            let av = *a.add(i);
            let bv = *b.add(i);
            if av < bv {
                return -1;
            } else if av > bv {
                return 1;
            }
        }
    }
    0
}

//
// Atomic bitmap operations
//
pub fn atomic_bitmap_set(bitmap: &AtomicU64, bit: u32) {
    bitmap.fetch_or(1u64 << bit, Ordering::SeqCst);
}

pub fn atomic_bitmap_clear(bitmap: &AtomicU64, bit: u32) {
    bitmap.fetch_and(!(1u64 << bit), Ordering::SeqCst);
}

pub fn atomic_bitmap_get(bitmap: &AtomicU64) -> u64 {
    bitmap.load(Ordering::SeqCst)
}

//
// Atomic reads
//
pub fn atomic_read8(v: &AtomicU8) -> u8 {
    v.load(Ordering::SeqCst)
}

pub fn atomic_read16(v: &AtomicU16) -> u16 {
    v.load(Ordering::SeqCst)
}

pub fn atomic_read32(v: &AtomicU32) -> u32 {
    v.load(Ordering::SeqCst)
}

pub fn atomic_read64(v: &AtomicU64) -> u64 {
    v.load(Ordering::SeqCst)
}

//
// Atomic writes
//
pub fn atomic_write8(v: &AtomicU8, val: u8) {
    v.store(val, Ordering::SeqCst);
}

pub fn atomic_write16(v: &AtomicU16, val: u16) {
    v.store(val, Ordering::SeqCst);
}

pub fn atomic_write32(v: &AtomicU32, val: u32) {
    v.store(val, Ordering::SeqCst);
}

pub fn atomic_write64(v: &AtomicU64, val: u64) {
    v.store(val, Ordering::SeqCst);
}

//
// Generic bitmap helpers
//
pub fn bitmap_generic_get(bitmap: &[u8], index: usize) -> bool {
    let div = index / 8;
    let bit = index % 8;
    (bitmap[div] & (1 << bit)) != 0
}

pub fn bitmap_generic_set(bitmap: &mut [u8], index: usize, set: bool) {
    let div = index / 8;
    let bit = index % 8;
    if set {
        bitmap[div] |= 1 << bit;
    } else {
        bitmap[div] &= !(1 << bit);
    }
}

//
// rand / srand (same LCG, same output)
//
static mut NEXT: u32 = 1;

pub fn rand() -> i32 {
    unsafe {
        NEXT = NEXT.wrapping_mul(1103515245).wrapping_add(12345);
        ((NEXT / 65536) % 32768) as i32
    }
}

pub fn srand(seed: u32) {
    unsafe {
        NEXT = seed;
    }
}

//
// Hex dump
//
pub fn hex_dump<F>(
    desc: Option<&str>,
    addr: *const u8,
    len: isize,
    per_line: usize,
    mut f: F,
)
where
    F: FnMut(&str),
{
    if let Some(d) = desc {
        f(&format!("{d}:\n"));
    }

    if len == 0 {
        f("  ZERO LENGTH\n");
        return;
    }
    if len < 0 {
        f(&format!("  NEGATIVE LENGTH: {len}\n"));
        return;
    }

    let len = len as usize;
    let mut buff = vec![0u8; per_line + 1];

    for i in 0..len {
        if i % per_line == 0 {
            if i != 0 {
                f(&format!("  {}\n", core::str::from_utf8(&buff[..per_line]).unwrap()));
            }
            f(&format!("  {:04x} ", i));
        }

        unsafe {
            let byte = *addr.add(i);
            f(&format!(" {:02x}", byte));

            buff[i % per_line] = if byte < 0x20 || byte > 0x7e {
                b'.'
            } else {
                byte
            };
        }
    }

    let mut i = len;
    while i % per_line != 0 {
        f("   ");
        i += 1;
    }

    f(&format!(
        "  {}\n",
        core::str::from_utf8(&buff[..per_line]).unwrap()
    ));
}
