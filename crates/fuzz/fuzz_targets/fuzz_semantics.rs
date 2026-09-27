//! Generated programs against the values they must compute, and every
//! other oracle on the way. See `rk_fuzz::generate`.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|case: &[u8]| {
    let program = rk_fuzz::generate::program(case);
    if let Err(finding) = rk_fuzz::check_all(&program) {
        panic!("{finding}\n--- the program ---\n{program}");
    }
});
