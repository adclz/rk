#![no_main]
use libfuzzer_sys::fuzz_target;

use auto_lsp::default::db::{BaseDatabase, FileManager, file::File};
use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use hir::hir_def::semantic_index::semantic_index;

fuzz_target!(|data: &[u8]| {
    // Skip empty or very large inputs
    if data.is_empty() || data.len() > 1_000_000 {
        return;
    }

    // Only fuzz valid UTF-8
    let source = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Create a fresh database for each fuzz run
    let mut db = RootDatabase::default();

    // Create a test file URL
    let url = match Url::parse("file:///fuzz_input.st") {
        Ok(u) => u,
        Err(_) => return,
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
        Err(_) => {
            // Parser errors are expected and not bugs
            return;
        }
    };

    // Add the file to the database
    if db.add_file(file).is_err() {
        return;
    }

    // Retrieve the file from the database
    let file = match db.get_file(&url) {
        Some(f) => f,
        None => return,
    };

    // Semantic analysis, where most bugs are.
    let _semantic_index = semantic_index(&db, file);

    // Diagnostics trigger type checking and name resolution.
    let _diagnostics = diagnostics_for_file(&db, file);
});
