#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::{FileManager, file::File};
use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;

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

    // Create a fresh database for each fuzz run
    let mut db = RootDatabase::default();

    // Create a test file URL
    let url = match Url::parse("file:///fuzz_input.st") {
        Ok(u) => u,
        Err(_) => return Corpus::Reject,
    };

    // Try to parse the input source code
    let file = match File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
    {
        Ok(f) => f,
        Err(_) => return Corpus::Reject,
    };

    // Add the file to the database
    if db.add_file(file).is_err() {
        return Corpus::Reject;
    }

    // Retrieve the file from the database
    let file = match db.get_file(&url) {
        Some(f) => f,
        None => return Corpus::Reject,
    };

    // Semantic analysis, where most bugs are.
    let _semantic_index = semantic_index(&db, file);

    // Diagnostics trigger type checking and name resolution.
    let _diagnostics = diagnostics_for_file(&db, file);
    Corpus::Keep
}

fuzz_target!(|case: &[u8]| -> Corpus { do_fuzz(case) });
