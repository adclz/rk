//! How a fault leaves a compiled PLC, and the checks that raise one.
//!
//! Every runtime check the compiler injects funnels through `__iec_raise`,
//! which throws the module's `$rk_exception` — the same throw `__RAISE`
//! lowers to, so a compiler-detected fault and a program-declared one are
//! indistinguishable to whoever catches them, and both carry a message.
//!
//! Wasm traps on its own for the faults its instruction set defines (a
//! division by zero, an address outside linear memory). Those are
//! uncatchable and speak in wasm's words. Everything a PLC needs beyond
//! them — an array subscript that would land on a neighbour, a value that
//! leaves its subrange — has no instruction to trap on, so it is checked
//! here, or not at all.

/// `no_std` panic landing for the builtins crate. Re-throws any Rust
/// panic as `$rk_exception` so user code can `TRY ... CATCH ... END_TRY`
/// it. `PanicMessage::as_str` returns `Some(&'static str)` when the
/// message has no format arguments — which covers every
/// compiler-generated panic that matters here (`"attempt to divide by
/// zero"`, `"attempt to divide with overflow"`, etc.) — and `None`
/// otherwise (e.g. slice-OOB, which interpolates the index). The
/// fallback message keeps the import wired up even on the rare
/// formatted-panic path; we deliberately avoid pulling `core::fmt::Write`
/// in just to handle it.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let msg: &'static str = info.message().as_str().unwrap_or("unknown error");
    unsafe { __iec_raise(msg.as_ptr(), msg.len() as u32) }
}

// Raise an IEC-level exception with a static message. The body codegen
// synthesizes at graft time is `local.get 0; local.get 1; throw
// $rk_exception; unreachable` — i.e. the same throw that `__RAISE`
// lowers to, with the (ptr, len) payload coming from the call.
//
// `__iec_raise` is the *single* path from Rust into the IEC exception
// system. Two ways to hit it:
//   1. **Explicit**: a builtin calls `rk_raise("specific message")`
//      when it has a tailored message it wants user code to see.
//   2. **Implicit**: any Rust `panic!()` — including compiler-generated
//      panics for `a / 0`, `INT_MIN / -1`, `slice[oob]`, `Option::unwrap`,
//      `-C overflow-checks=on` arithmetic — funnels through the
//      `#[panic_handler]` below, which formats the panic message and
//      calls `__iec_raise`. This means standard Rust runtime checks
//      surface as IEC exceptions for free, with Rust's native messages
//      (e.g. `"attempt to divide by zero"`).
unsafe extern "C" {
    fn __iec_raise(ptr: *const u8, len: u32) -> !;
}

/// Throw an `$rk_exception` carrying `msg` as the catchable payload.
/// `#[inline]` so the (ptr, len) flattening happens at the call site —
/// the bundle's stack pointer never has to spill a 16-byte slice header.
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

/// Null check for a RUNTIME dereference: raises an IEC exception when the
/// pointer is 0, else returns it unchanged.
///
/// Lowering wraps the pointer of every user-written `^`. Unchecked, a null
/// dereference was not a fault at all: a read answered 0 and a write silently
/// succeeded, and since `p^[i]` and `p^.field` address `0 + offset`, a large
/// enough offset reached past the reserved floor and corrupted live IEC
/// variables from a scan that reported nothing. `rk.idx_check` did not help
/// there — it validates the INDEX against the declared bounds and never sees
/// the base.
#[unsafe(no_mangle)]
pub extern "C" fn rk_null_check(ptr: i32) -> i32 {
    if ptr == 0 {
        const MSG: &str = "dereference of a null reference";
        unsafe { __iec_raise(MSG.as_ptr(), MSG.len() as u32) }
    }
    ptr
}

/// Range check for a RUNTIME value entering a subrange-typed slot: raises an
/// IEC exception when `value` leaves `[lower, upper]`, else returns it
/// unchanged. The compile-time counterpart is E0802, which catches the
/// constants; lowering wraps what E0802 cannot see.
///
/// Four variants, by lane and signedness, because the COMPARISON must match
/// the base type: a UDINT bound like 4_000_000_000 is a negative i32 bit
/// pattern, and only an unsigned compare reads it correctly. Sub-width bases
/// ride the i32 lane under the representation invariant (signed
/// sign-extended, unsigned zero-extended), so the same two comparisons hold.
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
