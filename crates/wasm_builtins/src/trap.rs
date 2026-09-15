//! How a fault leaves a compiled PLC: every runtime check funnels through
//! `__iec_raise`, which throws the module's `$rk_exception`, the same
//! throw `__RAISE` lowers to. Wasm traps on its own for its instruction
//! set's faults; a subscript landing on a neighbour or a value leaving its
//! subrange is checked here, or not at all.

/// `no_std` panic landing: re-throws any Rust panic as `$rk_exception`.
/// `PanicMessage::as_str` answers for every compiler-generated panic that
/// matters (`"attempt to divide by zero"`); a formatted panic gets the
/// fallback message.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let msg: &'static str = info.message().as_str().unwrap_or("unknown error");
    unsafe { __iec_raise(msg.as_ptr(), msg.len() as u32) }
}

// Raise an IEC exception with a static message: the body codegen
// synthesizes at graft time is the same throw `__RAISE` lowers to. The
// single path from Rust into the IEC exception system, reached explicitly
// by `rk_raise(...)` or implicitly by any Rust `panic!()` through the
// panic handler.
unsafe extern "C" {
    fn __iec_raise(ptr: *const u8, len: u32) -> !;
}

/// Throw an `$rk_exception` carrying `msg`; `#[inline]` so the (ptr, len)
/// flattening happens at the call site.
#[inline]
#[allow(dead_code)]
pub(crate) fn rk_raise(msg: &'static str) -> ! {
    unsafe { __iec_raise(msg.as_ptr(), msg.len() as u32) }
}

/// A live caller so wasm-ld keeps the `__iec_raise` import; callable by
/// name from grafted code to raise an arbitrary `(ptr, len)`.
#[unsafe(no_mangle)]
pub extern "C" fn rk_raise_str(ptr: *const u8, len: u32) -> () {
    unsafe { __iec_raise(ptr, len) }
}

/// Bounds check for a runtime array subscript: raises when `index` leaves
/// `[lower, lower + size)`, else returns it. The wrapping subtraction
/// folds the below-lower case into one unsigned comparison.
#[unsafe(no_mangle)]
pub extern "C" fn rk_idx_check(index: i32, lower: i32, size: u32) -> i32 {
    if (index.wrapping_sub(lower) as u32) >= size {
        const MSG: &str = "array index out of bounds";
        unsafe { __iec_raise(MSG.as_ptr(), MSG.len() as u32) }
    }
    index
}

/// Null check for a runtime dereference: raises when the pointer is 0,
/// else returns it. Lowering wraps every user-written `^`; `rk.idx_check`
/// validates the index and never sees the base.
#[unsafe(no_mangle)]
pub extern "C" fn rk_null_check(ptr: i32) -> i32 {
    if ptr == 0 {
        const MSG: &str = "dereference of a null reference";
        unsafe { __iec_raise(MSG.as_ptr(), MSG.len() as u32) }
    }
    ptr
}

/// Range check for a runtime value entering a subrange slot: raises when
/// `value` leaves `[lower, upper]`; the compile-time half is E0702. Four
/// variants by lane and signedness, since a UDINT bound like
/// 4_000_000_000 is a negative i32 bit pattern.
#[unsafe(no_mangle)]
pub extern "C" fn rk_range_check_i32(value: i32, lower: i32, upper: i32) -> i32 {
    if value < lower || value > upper {
        rk_raise("value out of subrange bounds");
    }
    value
}

#[unsafe(no_mangle)]
pub extern "C" fn rk_range_check_u32(value: u32, lower: u32, upper: u32) -> u32 {
    if value < lower || value > upper {
        rk_raise("value out of subrange bounds");
    }
    value
}

#[unsafe(no_mangle)]
pub extern "C" fn rk_range_check_i64(value: i64, lower: i64, upper: i64) -> i64 {
    if value < lower || value > upper {
        rk_raise("value out of subrange bounds");
    }
    value
}

#[unsafe(no_mangle)]
pub extern "C" fn rk_range_check_u64(value: u64, lower: u64, upper: u64) -> u64 {
    if value < lower || value > upper {
        rk_raise("value out of subrange bounds");
    }
    value
}

#[unsafe(no_mangle)]
pub extern "C" fn rk_div_i32_checked(numerator: i32, divisor: i32) -> i32 {
    numerator / divisor
}

#[unsafe(no_mangle)]
pub extern "C" fn rk_rem_i32_checked(numerator: i32, divisor: i32) -> i32 {
    numerator % divisor
}
