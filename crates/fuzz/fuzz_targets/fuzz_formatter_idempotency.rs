#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};

use formatter::TOPIARY_LANG;
use topiary_core::{Operation, formatter};

fn do_fuzz(case: &[u8]) -> Corpus {
    // Skip empty or very large inputs
    if case.is_empty() || case.len() > 1_000_000 {
        return Corpus::Reject;
    }

    // Only fuzz valid UTF-8
    let source = match std::str::from_utf8(case) {
        Ok(s) => s,
        Err(_) => return Corpus::Reject,
    };

    let mut formatted = vec![];
    match formatter(
        &mut source.as_bytes(),
        &mut formatted,
        &TOPIARY_LANG,
        Operation::Format {
            skip_idempotence: false,
            tolerate_parsing_errors: false,
        },
    ) {
        Ok(_) => (),
        Err(_) => return Corpus::Reject,
    }

    if formatted != source.as_bytes() {
        return Corpus::Reject;
    }

    let formatted = match String::from_utf8(formatted) {
        Err(_) => return Corpus::Reject,
        Ok(s) => s,
    };

    let mut reformatted = vec![];
    match formatter(
        &mut formatted.as_bytes(),
        &mut reformatted,
        &TOPIARY_LANG,
        Operation::Format {
            skip_idempotence: false,
            tolerate_parsing_errors: false,
        },
    ) {
        Ok(_) => (),
        Err(_) => return Corpus::Reject,
    }

    if reformatted != formatted.as_bytes() {
        return Corpus::Reject;
    }

    Corpus::Keep
}

fuzz_target!(|case: &[u8]| -> Corpus { do_fuzz(case) });
