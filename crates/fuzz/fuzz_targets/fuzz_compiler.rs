// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The compiler end to end: check, lint, lower, emit, validate, run.
//! See `rk_fuzz::pipeline`.
#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|case: &[u8]| -> Corpus {
    let Some(source) = rk_fuzz::as_source(case) else {
        return Corpus::Reject;
    };
    if let Err(finding) = rk_fuzz::pipeline::check(source) {
        panic!("{finding}");
    }
    Corpus::Keep
});
