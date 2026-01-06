#![no_std]
#![feature(c_variadic)]

use core::ffi::VaList;
use core::ptr::null_mut;

//
// ===== Output gadget =====
//

type PutCharFn = fn(u8, *mut core::ffi::c_void);

struct Output {
    putc: Option<PutCharFn>,
    arg: *mut core::ffi::c_void,
    buf: *mut u8,
    pos: usize,
    max: usize,
}

impl Output {
    fn write(&mut self, c: u8) {
        let p = self.pos;
        self.pos += 1;

        if p >= self.max {
            return;
        }

        unsafe {
            if let Some(f) = self.putc {
                f(c, self.arg);
            } else if !self.buf.is_null() {
                *self.buf.add(p) = c;
            }
        }
    }

    fn terminate(&mut self) {
        if self.buf.is_null() || self.max == 0 {
            return;
        }
        let idx = core::cmp::min(self.pos, self.max - 1);
        unsafe { *self.buf.add(idx) = 0 };
    }
}

//
// ===== Integer printing =====
//

fn print_uint(mut v: u64, base: u32, upper: bool, out: &mut Output) {
    let mut buf = [0u8; 32];
    let mut len = 0;

    if v == 0 {
        buf[len] = b'0';
        len += 1;
    } else {
        while v != 0 {
            let d = (v % base as u64) as u8;
            buf[len] = if d < 10 {
                b'0' + d
            } else {
                (if upper { b'A' } else { b'a' }) + (d - 10)
            };
            len += 1;
            v /= base as u64;
        }
    }

    for i in (0..len).rev() {
        out.write(buf[i]);
    }
}

fn print_int(v: i64, base: u32, out: &mut Output) {
    if v < 0 {
        out.write(b'-');
        print_uint((-v) as u64, base, false, out);
    } else {
        print_uint(v as u64, base, false, out);
    }
}

//
// ===== Format parser =====
//

unsafe fn vprintf_impl(out: &mut Output, fmt: *const u8, mut args: VaList) -> i32 {
    let mut p = fmt;

    while *p != 0 {
        if *p != b'%' {
            out.write(*p);
            p = p.add(1);
            continue;
        }

        p = p.add(1);

        match *p {
            b'd' | b'i' => {
                let v = args.arg::<i32>() as i64;
                print_int(v, 10, out);
            }
            b'u' => {
                let v = args.arg::<u32>() as u64;
                print_uint(v, 10, false, out);
            }
            b'x' => {
                let v = args.arg::<u32>() as u64;
                print_uint(v, 16, false, out);
            }
            b'X' => {
                let v = args.arg::<u32>() as u64;
                print_uint(v, 16, true, out);
            }
            b'o' => {
                let v = args.arg::<u32>() as u64;
                print_uint(v, 8, false, out);
            }
            b'c' => {
                let c = args.arg::<i32>() as u8;
                out.write(c);
            }
            b's' => {
                let mut s = args.arg::<*const u8>();
                if s.is_null() {
                    for &c in b"(null)" {
                        out.write(c);
                    }
                } else {
                    while *s != 0 {
                        out.write(*s);
                        s = s.add(1);
                    }
                }
            }
            b'p' => {
                out.write(b'0');
                out.write(b'x');
                let v = args.arg::<*const u8>() as usize as u64;
                print_uint(v, 16, false, out);
            }
            b'%' => out.write(b'%'),
            _ => {
                out.write(b'%');
                out.write(*p);
            }
        }

        p = p.add(1);
    }

    out.terminate();
    out.pos as i32
}

//
// ===== Public API =====
//

#[no_mangle]
pub unsafe extern "C" fn printf(format: *const u8, mut args: ...) -> i32 {
    extern "C" {
        fn putchar(c: u8);
    }

    fn wrapper(c: u8, _: *mut core::ffi::c_void) {
        unsafe { putchar(c) }
    }

    let mut out = Output {
        putc: Some(wrapper),
        arg: null_mut(),
        buf: null_mut(),
        pos: 0,
        max: usize::MAX,
    };

    vprintf_impl(&mut out, format, args.as_va_list())
}

#[no_mangle]
pub unsafe extern "C" fn vsnprintf(
    buf: *mut u8,
    size: usize,
    format: *const u8,
    args: VaList,
) -> i32 {
    let mut out = Output {
        putc: None,
        arg: null_mut(),
        buf,
        pos: 0,
        max: size,
    };

    vprintf_impl(&mut out, format, args)
}
