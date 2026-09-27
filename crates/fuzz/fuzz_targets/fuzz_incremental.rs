//! The editor's incremental path against a fresh build of the same text.
//! See `rk_fuzz::incremental`.
#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|case: &[u8]| -> Corpus {
    let Some(source) = rk_fuzz::as_source(case) else {
        return Corpus::Reject;
    };
    if let Err(finding) = rk_fuzz::incremental::check(source) {
        panic!("{finding}");
    }
    Corpus::Keep
});
