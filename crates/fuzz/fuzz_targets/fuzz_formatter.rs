// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The formatter: its output parses, is a fixed point, and means what the
//! input meant. See `rk_fuzz::format`.
#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|case: &[u8]| -> Corpus {
    let Some(source) = rk_fuzz::as_source(case) else {
        return Corpus::Reject;
    };
    if let Err(finding) = rk_fuzz::format::check(source) {
        panic!("{finding}");
    }
    Corpus::Keep
});
