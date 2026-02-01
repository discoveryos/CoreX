#![no_std]

/// Returns the length of a null-terminated string (like C's strlen)
pub fn strlength(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

/// Converts a string of digits to an integer (like C's atoi)
/// Only handles positive and negative integers; no error checking
pub fn atoi(s: *const u8) -> i32 {
    if s.is_null() {
        return 0;
    }
    let mut result: i32 = 0;
    let mut sign: i32 = 1;
    let mut idx = 0;

    unsafe {
        // Handle negative numbers
        if *s == b'-' {
            sign = -1;
            idx += 1;
        }

        while *s.add(idx) != 0 {
            let c = *s.add(idx);
            if c >= b'0' && c <= b'9' {
                result = result * 10 + ((c - b'0') as i32);
            } else {
                break; // stop at first non-digit
            }
            idx += 1;
        }
    }

    result * sign
}

/// Reverses a buffer of bytes in-place (like C's reverse)
pub fn reverse(buf: &mut [u8], len: usize) {
    if len == 0 {
        return;
    }
    let mut i = 0;
    let mut j = len - 1;

    while i < j {
        buf.swap(i, j);
        i += 1;
        j -= 1;
    }
}

/// Converts an integer to a null-terminated string (like C's itoa)
pub fn itoa(mut n: i32, buf: &mut [u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let mut i = 0;
    let sign = if n < 0 {
        n = -n;
        true
    } else {
        false
    };

    // Generate digits in reverse
    loop {
        buf[i] = b'0' + (n % 10) as u8;
        i += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }

    // Add sign if negative
    if sign {
        buf[i] = b'-';
        i += 1;
    }

    // Reverse digits
    reverse(buf, i);

    // Null terminate if space available
    if i < buf.len() {
        buf[i] = 0;
    }

    i
}
