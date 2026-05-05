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
