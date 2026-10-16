// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk fetch`: the libraries the workspace depends on, put in the cache.
//! `rk fetch --save <url>`: one more, written in its `config.toml` with the
//! hash of what the URL serves, so that nobody writes a hash by hand.

use std::path::Path;

use auto_lsp::lsp_types::Url;
use db::dependencies::{Fetch, Remote};
use db::stdlib::Standard;

use crate::cli::OutputFormat;
use crate::diagnostics::DiagnosticReporter;
use crate::error::{CliError, CliResult};
use crate::ui;

pub fn run_fetch(workspace: &Path, save: Option<&str>, format: OutputFormat) -> CliResult<()> {
    let Some(config_file) = db::loader::resolve_config_file(workspace) else {
        return Err(CliError::msg(format!(
            "no config.toml found in {}",
            workspace.display()
        )));
    };
    match save {
        Some(url) => save_dependency(&config_file, url),
        None => fetch_dependencies(workspace, &config_file, format),
    }
}

/// Fetches what `url` serves and writes it in `[dependencies]`.
fn save_dependency(config_file: &Path, url: &str) -> CliResult<()> {
    let cache = db::cache::root().ok_or_else(|| {
        CliError::msg(format!(
            "no cache directory: set {}",
            db::cache::CACHE_DIR_ENV
        ))
    })?;
    ui::detail(format!("    Fetching {url}"));
    let fetched = fetch::fetch(&cache, url).map_err(|error| {
        let mut message = format!("the dependency could not be fetched: {}", error.message);
        for note in &error.notes {
            message.push_str(&format!("\n    note: {note}"));
        }
        CliError::msg(message)
    })?;
    fetch::save(config_file, &fetched).map_err(CliError::msg)?;

    ui::success(
        "saved:",
        format!(
            "{} = {{ url = \"{}\", hash = \"{}\" }}",
            fetched.name, fetched.url, fetched.hash
        ),
    );
    Ok(())
}

/// Fetches each library the workspace depends on that the cache lacks, and
/// those they depend on. What cannot be resolved is reported on the entry
/// of `config.toml` it comes from.
fn fetch_dependencies(workspace: &Path, config_file: &Path, format: OutputFormat) -> CliResult<()> {
    let source = std::fs::read_to_string(config_file)
        .map_err(|e| CliError::msg(format!("reading {}: {e}", config_file.display())))?;
    let config = db::config_file::parse_config(&source)
        .map_err(|e| CliError::msg(format!("{}: {}", config_file.display(), e.message())))?;
    let standard = match db::stdlib::locate(Some(workspace)) {
        Standard::Found { library, .. } => Some(library),
        _ => None,
    };

    let cache = db::cache::root();
    let mut fetched = 0;
    let mut fetcher = crate::workspace::fetcher(true);
    let mut counting = |remote: &Remote<'_>| {
        let outcome = match fetcher.as_mut() {
            Some(fetch) => fetch(remote),
            None => Err(db::dependencies::FetchError::new("no cache directory")),
        };
        fetched += usize::from(outcome.is_ok());
        outcome
    };
    let fetch: Fetch<'_> = &mut counting;
    let resolution =
        db::dependencies::resolve(workspace, &config, standard, cache.as_deref(), Some(fetch));

    if !resolution.problems.is_empty() {
        let diagnostics: Vec<_> = resolution
            .problems
            .into_iter()
            .map(|problem| db::workspace::dependency_diagnostic(&source, problem))
            .collect();
        let db = db::RootDatabase::default();
        let url = Url::from_file_path(config_file)
            .map_err(|()| CliError::msg(format!("{} is not a path", config_file.display())))?;
        DiagnosticReporter::new(&db, workspace)
            .with_format(format)
            .report_external(&url, &source, &diagnostics, &mut std::io::stderr());
        return Err(CliError::Failed);
    }
    ui::success(
        "fetched:",
        match fetched {
            1 => "1 library".to_string(),
            n => format!("{n} libraries"),
        },
    );
    Ok(())
}
