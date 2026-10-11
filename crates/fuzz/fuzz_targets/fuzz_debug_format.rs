// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The debug tables through their reader, mutated: a seed is a compiled
//! program's custom sections, and nothing the reader is given may panic
//! it. See `rk_fuzz::debug::check`.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|case: &[u8]| {
    if let Err(finding) = rk_fuzz::debug::check(case) {
        panic!("{finding}");
    }
});
