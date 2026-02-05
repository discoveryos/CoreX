// OpenSSL-compatible MD5 implementation
// Ported from Alexander Peslyak's public-domain C implementation
// RFC 1321

#![allow(non_snake_case)]
#![allow(dead_code)]

use std::fmt::Write;

type MD5U32 = u32;

#[derive(Clone, Copy)]
struct MD5Ctx {
    a: MD5U32,
    b: MD5U32,
    c: MD5U32,
    d: MD5U32,
    lo: MD5U32,
    hi: MD5U32,
    buffer: [u8; 64],
    block: [MD5U32; 16],
}

impl Default for MD5Ctx {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            lo: 0,
            hi: 0,
            buffer: [0; 64],
            block: [0; 16],
        }
    }
}

/* Basic MD5 functions */
#[inline(always)]
fn F(x: MD5U32, y: MD5U32, z: MD5U32) -> MD5U32 {
    z ^ (x & (y ^ z))
}

#[inline(always)]
fn G(x: MD5U32, y: MD5U32, z: MD5U32) -> MD5U32 {
    y ^ (z & (x ^ y))
}

#[inline(always)]
fn H(x: MD5U32, y: MD5U32, z: MD5U32) -> MD5U32 {
    x ^ y ^ z
}

#[inline(always)]
fn H2(x: MD5U32, y: MD5U32, z: MD5U32) -> MD5U32 {
    x ^ (y ^ z)
}

#[inline(always)]
fn I(x: MD5U32, y: MD5U32, z: MD5U32) -> MD5U32 {
    y ^ (x | !z)
}

#[inline(always)]
fn step(
    f: fn(MD5U32, MD5U32, MD5U32) -> MD5U32,
    a: &mut MD5U32,
    b: MD5U32,
    c: MD5U32,
    d: MD5U32,
    x: MD5U32,
    t: MD5U32,
    s: u32,
) {
    *a = a
        .wrapping_add(f(b, c, d))
        .wrapping_add(x)
        .wrapping_add(t);
    *a = a.rotate_left(s);
    *a = a.wrapping_add(b);
}

#[inline(always)]
fn get_u32_le(input: &[u8], index: usize) -> MD5U32 {
    let i = index * 4;
    (input[i] as MD5U32)
        | ((input[i + 1] as MD5U32) << 8)
        | ((input[i + 2] as MD5U32) << 16)
        | ((input[i + 3] as MD5U32) << 24)
}

fn body(ctx: &mut MD5Ctx, data: &[u8]) {
    let mut a = ctx.a;
    let mut b = ctx.b;
    let mut c = ctx.c;
    let mut d = ctx.d;

    for chunk in data.chunks_exact(64) {
        let saved_a = a;
        let saved_b = b;
        let saved_c = c;
        let saved_d = d;

        for i in 0..16 {
            ctx.block[i] = get_u32_le(chunk, i);
        }

        /* Round 1 */
        step(F, &mut a, b, c, d, ctx.block[0], 0xd76aa478, 7);
        step(F, &mut d, a, b, c, ctx.block[1], 0xe8c7b756, 12);
        step(F, &mut c, d, a, b, ctx.block[2], 0x242070db, 17);
        step(F, &mut b, c, d, a, ctx.block[3], 0xc1bdceee, 22);
        step(F, &mut a, b, c, d, ctx.block[4], 0xf57c0faf, 7);
        step(F, &mut d, a, b, c, ctx.block[5], 0x4787c62a, 12);
        step(F, &mut c, d, a, b, ctx.block[6], 0xa8304613, 17);
        step(F, &mut b, c, d, a, ctx.block[7], 0xfd469501, 22);
        step(F, &mut a, b, c, d, ctx.block[8], 0x698098d8, 7);
        step(F, &mut d, a, b, c, ctx.block[9], 0x8b44f7af, 12);
        step(F, &mut c, d, a, b, ctx.block[10], 0xffff5bb1, 17);
        step(F, &mut b, c, d, a, ctx.block[11], 0x895cd7be, 22);
        step(F, &mut a, b, c, d, ctx.block[12], 0x6b901122, 7);
        step(F, &mut d, a, b, c, ctx.block[13], 0xfd987193, 12);
        step(F, &mut c, d, a, b, ctx.block[14], 0xa679438e, 17);
        step(F, &mut b, c, d, a, ctx.block[15], 0x49b40821, 22);

        /* Round 2 */
        step(G, &mut a, b, c, d, ctx.block[1], 0xf61e2562, 5);
        step(G, &mut d, a, b, c, ctx.block[6], 0xc040b340, 9);
        step(G, &mut c, d, a, b, ctx.block[11], 0x265e5a51, 14);
        step(G, &mut b, c, d, a, ctx.block[0], 0xe9b6c7aa, 20);
        step(G, &mut a, b, c, d, ctx.block[5], 0xd62f105d, 5);
        step(G, &mut d, a, b, c, ctx.block[10], 0x02441453, 9);
        step(G, &mut c, d, a, b, ctx.block[15], 0xd8a1e681, 14);
        step(G, &mut b, c, d, a, ctx.block[4], 0xe7d3fbc8, 20);
        step(G, &mut a, b, c, d, ctx.block[9], 0x21e1cde6, 5);
        step(G, &mut d, a, b, c, ctx.block[14], 0xc33707d6, 9);
        step(G, &mut c, d, a, b, ctx.block[3], 0xf4d50d87, 14);
        step(G, &mut b, c, d, a, ctx.block[8], 0x455a14ed, 20);
        step(G, &mut a, b, c, d, ctx.block[13], 0xa9e3e905, 5);
        step(G, &mut d, a, b, c, ctx.block[2], 0xfcefa3f8, 9);
        step(G, &mut c, d, a, b, ctx.block[7], 0x676f02d9, 14);
        step(G, &mut b, c, d, a, ctx.block[12], 0x8d2a4c8a, 20);

        /* Round 3 */
        step(H, &mut a, b, c, d, ctx.block[5], 0xfffa3942, 4);
        step(H2, &mut d, a, b, c, ctx.block[8], 0x8771f681, 11);
        step(H, &mut c, d, a, b, ctx.block[11], 0x6d9d6122, 16);
        step(H2, &mut b, c, d, a, ctx.block[14], 0xfde5380c, 23);
        step(H, &mut a, b, c, d, ctx.block[1], 0xa4beea44, 4);
        step(H2, &mut d, a, b, c, ctx.block[4], 0x4bdecfa9, 11);
        step(H, &mut c, d, a, b, ctx.block[7], 0xf6bb4b60, 16);
        step(H2, &mut b, c, d, a, ctx.block[10], 0xbebfbc70, 23);
        step(H, &mut a, b, c, d, ctx.block[13], 0x289b7ec6, 4);
        step(H2, &mut d, a, b, c, ctx.block[0], 0xeaa127fa, 11);
        step(H, &mut c, d, a, b, ctx.block[3], 0xd4ef3085, 16);
        step(H2, &mut b, c, d, a, ctx.block[6], 0x04881d05, 23);
        step(H, &mut a, b, c, d, ctx.block[9], 0xd9d4d039, 4);
        step(H2, &mut d, a, b, c, ctx.block[12], 0xe6db99e5, 11);
        step(H, &mut c, d, a, b, ctx.block[15], 0x1fa27cf8, 16);
        step(H2, &mut b, c, d, a, ctx.block[2], 0xc4ac5665, 23);

        /* Round 4 */
        step(I, &mut a, b, c, d, ctx.block[0], 0xf4292244, 6);
        step(I, &mut d, a, b, c, ctx.block[7], 0x432aff97, 10);
        step(I, &mut c, d, a, b, ctx.block[14], 0xab9423a7, 15);
        step(I, &mut b, c, d, a, ctx.block[5], 0xfc93a039, 21);
        step(I, &mut a, b, c, d, ctx.block[12], 0x655b59c3, 6);
        step(I, &mut d, a, b, c, ctx.block[3], 0x8f0ccc92, 10);
        step(I, &mut c, d, a, b, ctx.block[10], 0xffeff47d, 15);
        step(I, &mut b, c, d, a, ctx.block[1], 0x85845dd1, 21);
        step(I, &mut a, b, c, d, ctx.block[8], 0x6fa87e4f, 6);
        step(I, &mut d, a, b, c, ctx.block[15], 0xfe2ce6e0, 10);
        step(I, &mut c, d, a, b, ctx.block[6], 0xa3014314, 15);
        step(I, &mut b, c, d, a, ctx.block[13], 0x4e0811a1, 21);
        step(I, &mut a, b, c, d, ctx.block[4], 0xf7537e82, 6);
        step(I, &mut d, a, b, c, ctx.block[11], 0xbd3af235, 10);
        step(I, &mut c, d, a, b, ctx.block[2], 0x2ad7d2bb, 15);
        step(I, &mut b, c, d, a, ctx.block[9], 0xeb86d391, 21);

        a = a.wrapping_add(saved_a);
        b = b.wrapping_add(saved_b);
        c = c.wrapping_add(saved_c);
        d = d.wrapping_add(saved_d);
    }

    ctx.a = a;
    ctx.b = b;
    ctx.c = c;
    ctx.d = d;
}

pub fn md5_init(ctx: &mut MD5Ctx) {
    ctx.a = 0x67452301;
    ctx.b = 0xefcdab89;
    ctx.c = 0x98badcfe;
    ctx.d = 0x10325476;
    ctx.lo = 0;
    ctx.hi = 0;
}

pub fn md5_update(ctx: &mut MD5Ctx, data: &[u8]) {
    let saved_lo = ctx.lo;
    ctx.lo = ctx.lo.wrapping_add(data.len() as u32);
    if ctx.lo < saved_lo {
        ctx.hi = ctx.hi.wrapping_add(1);
    }
    ctx.hi = ctx.hi.wrapping_add((data.len() as u32) >> 29);

    let used = (saved_lo & 0x3f) as usize;
    let mut offset = 0;

    if used != 0 {
        let available = 64 - used;
        if data.len() < available {
            ctx.buffer[used..used + data.len()].copy_from_slice(data);
            return;
        }
        ctx.buffer[used..64].copy_from_slice(&data[..available]);
        body(ctx, &ctx.buffer);
        offset += available;
    }

    let remaining = data.len() - offset;
    let blocks = remaining & !0x3f;
    if blocks > 0 {
        body(ctx, &data[offset..offset + blocks]);
        offset += blocks;
    }

    ctx.buffer[..remaining - (remaining & !0x3f)]
        .copy_from_slice(&data[offset..]);
}

pub fn md5_final(ctx: &mut MD5Ctx) -> [u8; 16] {
    let used = (ctx.lo & 0x3f) as usize;
    ctx.buffer[used] = 0x80;

    let available = 64 - used;
    if available < 8 {
        for b in &mut ctx.buffer[used + 1..] {
            *b = 0;
        }
        body(ctx, &ctx.buffer);
        ctx.buffer.fill(0);
    } else {
        for b in &mut ctx.buffer[used + 1..56] {
            *b = 0;
        }
    }

    let bit_len_lo = ctx.lo << 3;
    let bit_len_hi = ctx.hi;

    ctx.buffer[56..60].copy_from_slice(&bit_len_lo.to_le_bytes());
    ctx.buffer[60..64].copy_from_slice(&bit_len_hi.to_le_bytes());

    body(ctx, &ctx.buffer);

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&ctx.a.to_le_bytes());
    out[4..8].copy_from_slice(&ctx.b.to_le_bytes());
    out[8..12].copy_from_slice(&ctx.c.to_le_bytes());
    out[12..16].copy_from_slice(&ctx.d.to_le_bytes());

    *ctx = MD5Ctx::default();
    out
}

/// Equivalent of MD5_Simple
pub fn md5_simple(data: &[u8]) -> String {
    let mut ctx = MD5Ctx::default();
    md5_init(&mut ctx);
    md5_update(&mut ctx, data);
    let digest = md5_final(&mut ctx);

    let mut out = String::with_capacity(32);
    for b in digest {
        write!(&mut out, "{:02x}", b).unwrap();
    }
    out
}
