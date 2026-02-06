

use core::ptr;

pub fn strlength(ch: *const u8) -> usize {
    let mut i = 0;
    unsafe {
        while *ch.add(i) != 0 {
            i += 1;
        }
    }
    i
}

pub fn strlen(ch: *const u8) -> usize {
    strlength(ch)
}

pub fn strncmp(str1: *const u8, str2: *const u8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let c1 = *str1.add(i);
            let c2 = *str2.add(i);

            if c1 == 0 || c2 == 0 {
                return c1 as i32 - c2 as i32;
            }
            if c1 != c2 {
                return c1 as i32 - c2 as i32;
            }
        }
    }
    0
}

pub unsafe fn strdup(source: *const u8) -> *mut u8 {
    let len = strlength(source) + 1;
    let target = alloc(len);
    ptr::copy_nonoverlapping(source, target, len);
    target
}

// very small allocator wrapper (replace with your own)
unsafe fn alloc(size: usize) -> *mut u8 {
    extern "C" {
        fn malloc(size: usize) -> *mut u8;
    }
    malloc(size)
}

pub fn strncpy(dest: *mut u8, src: *const u8, n: usize) {
    unsafe {
        let mut i = 0;
        while i < n && *src.add(i) != 0 {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
        while i < n {
            *dest.add(i) = 0;
            i += 1;
        }
    }
}

pub fn isdigit(c: u8) -> bool {
    c >= b'0' && c <= b'9'
}

pub fn atoi(mut str: *const u8) -> i32 {
    let mut value = 0;
    unsafe {
        while isdigit(*str) {
            value = value * 10 + (*str - b'0') as i32;
            str = str.add(1);
        }
    }
    value
}

pub fn num_at_end(str: *const u8) -> u64 {
    unsafe {
        let mut p = str;
        while *p != 0 {
            p = p.add(1);
        }

        let end = p;

        while p > str && {
            let c = *p.sub(1);
            c >= b'0' && c <= b'9'
        } {
            p = p.sub(1);
        }

        if p == end {
            return 0;
        }

        let mut num: u64 = 0;
        while p < end {
            num = num * 10 + (*p - b'0') as u64;
            p = p.add(1);
        }

        num
    }
}

pub fn check_string(str: *const u8) -> bool {
    unsafe { *str != 0 }
}

pub fn str_eql(ch1: *const u8, ch2: *const u8) -> bool {
    let size1 = strlength(ch1);
    let size2 = strlength(ch2);

    if size1 != size2 {
        return false;
    }

    unsafe {
        for i in 0..=size1 {
            if *ch1.add(i) != *ch2.add(i) {
                return false;
            }
        }
    }
    true
}

pub fn strpbrk(str: *const u8, delimiters: *const u8) -> *mut u8 {
    unsafe {
        let mut s = str;
        while *s != 0 {
            let mut d = delimiters;
            while *d != 0 {
                if *d == *s {
                    return s as *mut u8;
                }
                d = d.add(1);
            }
            s = s.add(1);
        }
    }
    ptr::null_mut()
}

pub fn strtok(
    str: *mut u8,
    delimiters: *const u8,
    context: &mut *mut u8,
) -> *mut u8 {
    unsafe {
        if str.is_null() && context.is_null() {
            return ptr::null_mut();
        }

        if !str.is_null() {
            *context = str;
        }

        let token_start = *context;
        if token_start.is_null() {
            return ptr::null_mut();
        }

        let token_end = strpbrk(token_start, delimiters);

        if !token_end.is_null() {
            *token_end = 0;
            *context = token_end.add(1);
            token_start
        } else if *token_start != 0 {
            *context = ptr::null_mut();
            token_start
        } else {
            ptr::null_mut()
        }
    }
}

pub fn strtol(mut s: *const u8, endptr: *mut *const u8, mut base: i32) -> i64 {
    let mut neg = false;
    let mut val: i64 = 0;

    unsafe {
        while *s == b' ' || *s == b'\t' {
            s = s.add(1);
        }

        if *s == b'+' {
            s = s.add(1);
        } else if *s == b'-' {
            neg = true;
            s = s.add(1);
        }

        if (base == 0 || base == 16) && *s == b'0' && *s.add(1) == b'x' {
            s = s.add(2);
            base = 16;
        } else if base == 0 && *s == b'0' {
            s = s.add(1);
            base = 8;
        } else if base == 0 {
            base = 10;
        }

        loop {
            let dig = match *s {
                b'0'..=b'9' => (*s - b'0') as i32,
                b'a'..=b'z' => (*s - b'a' + 10) as i32,
                b'A'..=b'Z' => (*s - b'A' + 10) as i32,
                _ => break,
            };

            if dig >= base {
                break;
            }

            val = val * base as i64 + dig as i64;
            s = s.add(1);
        }

        if !endptr.is_null() {
            *endptr = s;
        }
    }

    if neg { -val } else { val }
}

pub fn strrchr(str: *const u8, c: u8) -> *mut u8 {
    let mut last: *const u8 = ptr::null();
    unsafe {
        let mut s = str;
        while *s != 0 {
            if *s == c {
                last = s;
            }
            s = s.add(1);
        }
    }
    last as *mut u8
}
