// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk fetch`: the libraries the workspace depends on, put in the cache.
//! `rk fetch <url>`: one more, added to its `config.toml` with the hash of
//! what the URL serves, so that nobody writes a hash by hand. It is checked
//! first, as the build would check it, and nothing is written when it fails.

use std::path::Path;

use auto_lsp::lsp_types::Url;
use db::dependencies::{Fetch, Remote};
use db::stdlib::Standard;

use crate::cli::OutputFormat;
use crate::diagnostics::DiagnosticReporter;
use crate::error::{CliError, CliResult};
use crate::ui;

pub fn run_fetch(workspace: &Path, url: Option<&str>, format: OutputFormat) -> CliResult<()> {
    let Some(config_file) = db::loader::resolve_config_file(workspace) else {
        return Err(CliError::msg(format!(
            "no config.toml found in {}",
            workspace.display()
        )));
    };
    match url {
        Some(url) => add_dependency(workspace, &config_file, url, format),
        None => fetch_dependencies(workspace, &config_file, format),
    }
}

/// Fetches what `url` serves and adds it to `[dependencies]`, or moves the
/// entry of its name to it. The entry is checked as the build checks it,
/// and the library's code as `rk check` checks it in its own folder:
/// nothing is written unless both pass.
fn add_dependency(
    workspace: &Path,
    config_file: &Path,
    url: &str,
    format: OutputFormat,
) -> CliResult<()> {
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
    let line = format!(
        "{} = {{ url = \"{}\", hash = \"{}\" }}",
        fetched.name, fetched.url, fetched.hash
    );

    let source = std::fs::read_to_string(config_file)
        .map_err(|e| CliError::msg(format!("reading {}: {e}", config_file.display())))?;
    let config = db::config_file::parse_config(&source)
        .map_err(|e| CliError::msg(format!("{}: {}", config_file.display(), e.message())))?;
    let hash = fetched.hash.to_string();
    if config.dependencies.iter().any(|(name, entry)| {
        name.get_ref() == &fetched.name
            && entry.url.as_deref() == Some(fetched.url.as_str())
            && entry.hash.as_deref() == Some(hash.as_str())
    }) {
        ui::success("up to date:", line);
        return Ok(());
    }

    // The entry as it would be written, resolved as the build resolves it:
    // a name, a library, no dependencies of its own, no other of its name.
    let written = fetch::with_entry(&source, &fetched)
        .map_err(|e| CliError::msg(format!("{}: {e}", config_file.display())))?;
    let would_be = db::config_file::parse_config(&written)
        .map_err(|e| CliError::msg(format!("{}: {}", config_file.display(), e.message())))?;
    let entry = would_be
        .dependencies
        .keys()
        .find(|name| name.get_ref() == &fetched.name)
        .map(|name| name.span());
    let standard = match db::stdlib::locate(Some(workspace)) {
        Standard::Found { library, .. } => Some(library),
        _ => None,
    };
    let problems: Vec<_> =
        db::dependencies::resolve(workspace, &would_be, standard, Some(&cache), None)
            .problems
            .into_iter()
            .filter(|problem| Some(&problem.span) == entry.as_ref())
            .map(|problem| db::workspace::dependency_diagnostic(&written, problem))
            .collect();
    if !problems.is_empty() {
        let db = db::RootDatabase::default();
        let config_url = Url::from_file_path(config_file)
            .map_err(|()| CliError::msg(format!("{} is not a path", config_file.display())))?;
        DiagnosticReporter::new(&db, workspace)
            .with_format(format)
            .report_external(&config_url, &written, &problems, &mut std::io::stderr());
        ui::failure("not added:", line);
        return Err(CliError::Failed);
    }

    // The library's own code, held to a library's rules.
    ui::detail(format!("    Checking {}", fetched.name));
    if !library_is_clean(&fetched.dir, format) {
        ui::failure("not added:", line);
        return Err(CliError::Failed);
    }

    std::fs::write(config_file, written)
        .map_err(|e| CliError::msg(format!("writing {}: {e}", config_file.display())))?;
    ui::success("added:", line);
    Ok(())
}

/// Whether the library at `dir` checks with no error, as `rk check` checks
/// it in its own folder, where it keeps a library's rules. Its errors are
/// reported.
fn library_is_clean(dir: &Path, format: OutputFormat) -> bool {
    let Some(db) = crate::workspace::init_db(dir, false, true) else {
        return false;
    };
    let errors = crate::diagnostics::collect_errors(&db);
    if errors.is_empty() {
        return true;
    }
    DiagnosticReporter::new(&db, dir)
        .with_format(format)
        .report_files(&errors, &mut std::io::stderr());
    false
}

/// Fetches each library the workspace depends on that the cache lacks.
/// What cannot be resolved is reported on the entry of `config.toml` it
/// comes from.
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
