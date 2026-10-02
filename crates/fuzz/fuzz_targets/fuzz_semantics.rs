// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Generated programs against the values they must compute, and every
//! other oracle but the language server's on the way. See
//! `rk_fuzz::generate` and `rk_fuzz::check_generated`.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|case: &[u8]| {
    let program = rk_fuzz::generate::program(case);
    if let Err(finding) = rk_fuzz::check_generated(&program) {
        panic!("{finding}\n--- the program ---\n{program}");
    }
});
