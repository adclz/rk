//! Pre-compiled WASM math intrinsics for the IEC stdlib, built to
//! `wasm32-unknown-unknown` by `wasm_codegen`'s build.rs and grafted on
//! demand via `{wasm IN 'sin' ...}` pragmas. Export names are
//! `<wasm_type>_<op>` (`f32.sin` → `f32_sin`).

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_sin(x: f32) -> f32 {
    libm::sinf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_sin(x: f64) -> f64 {
    libm::sin(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_cos(x: f32) -> f32 {
    libm::cosf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_cos(x: f64) -> f64 {
    libm::cos(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_tan(x: f32) -> f32 {
    libm::tanf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_tan(x: f64) -> f64 {
    libm::tan(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_asin(x: f32) -> f32 {
    libm::asinf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_asin(x: f64) -> f64 {
    libm::asin(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_acos(x: f32) -> f32 {
    libm::acosf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_acos(x: f64) -> f64 {
    libm::acos(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_atan(x: f32) -> f32 {
    libm::atanf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_atan(x: f64) -> f64 {
    libm::atan(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_exp(x: f32) -> f32 {
    libm::expf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_exp(x: f64) -> f64 {
    libm::exp(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_ln(x: f32) -> f32 {
    libm::logf(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_ln(x: f64) -> f64 {
    libm::log(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f32_log(x: f32) -> f32 {
    libm::log10f(x)
}
#[unsafe(no_mangle)]
pub extern "C" fn f64_log(x: f64) -> f64 {
    libm::log10(x)
}
// The IEC ABI for STRING is `(ptr: i32, len: i32)`, two scalars;
// `extern "C" fn(s: &[u8])` would pass a slice header by pointer, so the
// pair is spelled out.

/// A `&[u8]` from raw FFI parts, defending against `(null, 0)` (a static
/// empty slice) and `len > isize::MAX` (clamped).
#[inline]
unsafe fn ffi_slice<'a>(ptr: *const u8, len: u32) -> &'a [u8] {
    if len == 0 {
        return &[];
    }
    let clamped = (len as usize).min(isize::MAX as usize);
    unsafe { core::slice::from_raw_parts(ptr, clamped) }
}

/// Assign a `(src_ptr, src_len)` string into a fixed-capacity STRING at
/// `dest_addr` (length at +0, buffer at +4), truncating to `dest_cap`.
/// Called at every `string_var := <expr>`.
#[unsafe(no_mangle)]
pub extern "C" fn rk_str_assign(
    dest_addr: u32,
    dest_cap: u32,
    src_ptr: *const u8,
    src_len: u32,
) {
    let take = src_len.min(dest_cap);
    unsafe {
        // Write the new length into the header.
        (dest_addr as *mut u32).write_unaligned(take);
        // Copy bytes into the embedded buffer (just past the header).
        if take > 0 {
            core::ptr::copy_nonoverlapping(
                src_ptr,
                (dest_addr + 4) as *mut u8,
                take as usize,
            );
        }
    }
}

/// Length in raw bytes - STRINGs are stored as `(ptr, len)`, this just
/// returns the second word. Always succeeds.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_len(_ptr: *const u8, len: u32) -> u32 {
    len
}

/// Validate UTF-8. `1` if the input is well-formed UTF-8, `0` otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_utf8(ptr: *const u8, len: u32) -> i32 {
    let bytes = unsafe { ffi_slice(ptr, len) };
    if core::str::from_utf8(bytes).is_ok() { 1 } else { 0 }
}

/// 1-indexed byte position of `needle` in `haystack`, `0` if absent, `1`
/// for an empty needle. Byte-level.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_find(
    haystack_ptr: *const u8,
    haystack_len: u32,
    needle_ptr: *const u8,
    needle_len: u32,
) -> u32 {
    if needle_len == 0 {
        return 1;
    }
    let haystack = unsafe { ffi_slice(haystack_ptr, haystack_len) };
    let needle = unsafe { ffi_slice(needle_ptr, needle_len) };
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
        .map_or(0, |i| (i as u32) + 1)
}

// ---------------------------------------------------------------------------
// String producers: each takes `(out_addr, out_cap, ...args)` and writes
// the result into the caller's STRING storage (length then buffer),
// truncating to `out_cap` rather than trapping.
// ---------------------------------------------------------------------------

/// Common helper: write `bytes` into the buffer at `out_addr + 4`,
/// truncated to `out_cap`, and store the actual length in `*out_addr`.
#[inline]
unsafe fn str_emit_bytes(out_addr: u32, out_cap: u32, bytes: &[u8]) {
    let take = (bytes.len() as u32).min(out_cap);
    unsafe {
        (out_addr as *mut u32).write_unaligned(take);
        if take > 0 {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (out_addr + 4) as *mut u8,
                take as usize,
            );
        }
    }
}

/// `dest := CONCAT(a, b)` - append `b` after `a`, truncate at `dest`'s
/// capacity. Bytes past the cap are silently dropped.
#[unsafe(no_mangle)]
pub extern "C" fn str_concat(
    a_ptr: *const u8,
    a_len: u32,
    b_ptr: *const u8,
    b_len: u32,
    out_addr: u32,
    out_cap: u32,
) {
    let a = unsafe { ffi_slice(a_ptr, a_len) };
    let b = unsafe { ffi_slice(b_ptr, b_len) };
    let take_a = (a.len() as u32).min(out_cap);
    let take_b = (b.len() as u32).min(out_cap - take_a);
    let total = take_a + take_b;
    unsafe {
        (out_addr as *mut u32).write_unaligned(total);
        if take_a > 0 {
            core::ptr::copy_nonoverlapping(
                a.as_ptr(),
                (out_addr + 4) as *mut u8,
                take_a as usize,
            );
        }
        if take_b > 0 {
            core::ptr::copy_nonoverlapping(
                b.as_ptr(),
                (out_addr + 4 + take_a) as *mut u8,
                take_b as usize,
            );
        }
    }
}

/// `dest := LEFT(s, n)`: the first `n` bytes of `s`, or the whole string
/// when `n > len(s)`. Byte-level.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_left(s_ptr: *const u8, s_len: u32, n: u32, out_addr: u32, out_cap: u32) {
    let s = unsafe { ffi_slice(s_ptr, s_len) };
    let take = n.min(s_len);
    unsafe { str_emit_bytes(out_addr, out_cap, &s[..take as usize]) };
}

/// `dest := RIGHT(s, n)` - last `n` bytes of `s`. Mirrors LEFT.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_right(
    s_ptr: *const u8,
    s_len: u32,
    n: u32,
    out_addr: u32,
    out_cap: u32,
) {
    let s = unsafe { ffi_slice(s_ptr, s_len) };
    let take = n.min(s_len);
    let start = (s_len - take) as usize;
    unsafe { str_emit_bytes(out_addr, out_cap, &s[start..]) };
}

/// `dest := MID(s, n, p)` - `n` bytes starting at 1-indexed byte
/// position `p`. Empty result when `p` exceeds the source length.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_mid(
    s_ptr: *const u8,
    s_len: u32,
    n: u32,
    p: u32,
    out_addr: u32,
    out_cap: u32,
) {
    if p == 0 || p > s_len {
        unsafe { str_emit_bytes(out_addr, out_cap, &[]) };
        return;
    }
    let s = unsafe { ffi_slice(s_ptr, s_len) };
    let start = (p - 1) as usize;
    let end = (start + n as usize).min(s.len());
    unsafe { str_emit_bytes(out_addr, out_cap, &s[start..end]) };
}

/// `dest := INSERT(s, ins, p)` - insert `ins` into `s` after byte
/// position `p` (0 inserts at the start). Truncates to fit `out_cap`.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_insert(
    s_ptr: *const u8,
    s_len: u32,
    ins_ptr: *const u8,
    ins_len: u32,
    p: u32,
    out_addr: u32,
    out_cap: u32,
) {
    let s = unsafe { ffi_slice(s_ptr, s_len) };
    let ins = unsafe { ffi_slice(ins_ptr, ins_len) };
    let split = (p as usize).min(s.len());
    // Three slices, left part, `ins`, right part, each truncated against
    // the remaining capacity.
    let take_left = (split as u32).min(out_cap);
    let take_ins = (ins.len() as u32).min(out_cap - take_left);
    let take_right =
        ((s.len() - split) as u32).min(out_cap - take_left - take_ins);
    let total = take_left + take_ins + take_right;
    unsafe {
        (out_addr as *mut u32).write_unaligned(total);
        let buf = (out_addr + 4) as *mut u8;
        if take_left > 0 {
            core::ptr::copy_nonoverlapping(s.as_ptr(), buf, take_left as usize);
        }
        if take_ins > 0 {
            core::ptr::copy_nonoverlapping(
                ins.as_ptr(),
                buf.add(take_left as usize),
                take_ins as usize,
            );
        }
        if take_right > 0 {
            core::ptr::copy_nonoverlapping(
                s.as_ptr().add(split),
                buf.add((take_left + take_ins) as usize),
                take_right as usize,
            );
        }
    }
}

/// `dest := DELETE(s, n, p)`: remove `n` bytes at 1-indexed position `p`;
/// `p == 0` or past the end copies `s` verbatim.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_delete(
    s_ptr: *const u8,
    s_len: u32,
    n: u32,
    p: u32,
    out_addr: u32,
    out_cap: u32,
) {
    let s = unsafe { ffi_slice(s_ptr, s_len) };
    if p == 0 || p > s_len {
        unsafe { str_emit_bytes(out_addr, out_cap, s) };
        return;
    }
    let start = (p - 1) as usize;
    let end = (start + n as usize).min(s.len());
    let take_left = (start as u32).min(out_cap);
    let take_right = ((s.len() - end) as u32).min(out_cap - take_left);
    let total = take_left + take_right;
    unsafe {
        (out_addr as *mut u32).write_unaligned(total);
        let buf = (out_addr + 4) as *mut u8;
        if take_left > 0 {
            core::ptr::copy_nonoverlapping(s.as_ptr(), buf, take_left as usize);
        }
        if take_right > 0 {
            core::ptr::copy_nonoverlapping(
                s.as_ptr().add(end),
                buf.add(take_left as usize),
                take_right as usize,
            );
        }
    }
}

/// `dest := REPLACE(s, ins, n, p)`: `ins` in place of `n` bytes at
/// 1-indexed position `p`; `p == 0` returns `s` unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn str_byte_replace(
    s_ptr: *const u8,
    s_len: u32,
    ins_ptr: *const u8,
    ins_len: u32,
    n: u32,
    p: u32,
    out_addr: u32,
    out_cap: u32,
) {
    let s = unsafe { ffi_slice(s_ptr, s_len) };
    let ins = unsafe { ffi_slice(ins_ptr, ins_len) };
    if p == 0 || p > s_len {
        unsafe { str_emit_bytes(out_addr, out_cap, s) };
        return;
    }
    let start = (p - 1) as usize;
    let end = (start + n as usize).min(s.len());
    let take_left = (start as u32).min(out_cap);
    let take_ins = (ins.len() as u32).min(out_cap - take_left);
    let take_right =
        ((s.len() - end) as u32).min(out_cap - take_left - take_ins);
    let total = take_left + take_ins + take_right;
    unsafe {
        (out_addr as *mut u32).write_unaligned(total);
        let buf = (out_addr + 4) as *mut u8;
        if take_left > 0 {
            core::ptr::copy_nonoverlapping(s.as_ptr(), buf, take_left as usize);
        }
        if take_ins > 0 {
            core::ptr::copy_nonoverlapping(
                ins.as_ptr(),
                buf.add(take_left as usize),
                take_ins as usize,
            );
        }
        if take_right > 0 {
            core::ptr::copy_nonoverlapping(
                s.as_ptr().add(end),
                buf.add((take_left + take_ins) as usize),
                take_right as usize,
            );
        }
    }
}

