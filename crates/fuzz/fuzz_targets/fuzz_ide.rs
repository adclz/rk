//! Every language server request, at positions spread over the input.
//! See `rk_fuzz::ide`.
#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|case: &[u8]| -> Corpus {
    let Some(source) = rk_fuzz::as_source(case) else {
        return Corpus::Reject;
    };
    if let Err(finding) = rk_fuzz::ide::check(source) {
        panic!("{finding}");
    }
    Corpus::Keep
});
