// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `rk clean`: deletes the workspace's build folder in the cache, all that
//! `rk compile` and `rk test` wrote for it. The libraries the cache holds
//! stay, and so do the other workspaces' builds.

use std::path::Path;

use crate::error::{CliError, CliResult};
use crate::ui;

pub fn run_clean(workspace: &Path) -> CliResult<()> {
    // With no cache, nothing could be built.
    let build = db::cache::root().map(|cache| db::cache::build_dir(&cache, workspace));
    let Some(build) = build.filter(|build| build.exists()) else {
        ui::success("cleaned:", "nothing was built for this workspace");
        return Ok(());
    };
    let (files, bytes) = held(&build);
    db::cache::remove(&build)
        .map_err(|e| CliError::msg(format!("removing {}: {e}", build.display())))?;
    let files = match files {
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    };
    ui::success(
        "cleaned:",
        format!("{} ({files}, {bytes} bytes)", build.display()),
    );
    Ok(())
}

/// How many files `dir` holds, in it and in its folders, and their bytes.
fn held(dir: &Path) -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    let (mut files, mut bytes) = (0, 0);
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let (inner_files, inner_bytes) = held(&entry.path());
            files += inner_files;
            bytes += inner_bytes;
        } else {
            files += 1;
            bytes += entry.metadata().map_or(0, |metadata| metadata.len());
        }
    }
    (files, bytes)
}
