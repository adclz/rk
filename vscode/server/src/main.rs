/*
This file is part of auto-lsp.
Copyright (C) 2025 CLAUZEL Adrien

auto-lsp is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>
*/
use std::error::Error;

use server::boot;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Quiet unless asked: `RUST_LOG=debug` shows per-file work, `trace` every
    // lookup. A debug build used to default to `info`, and the instrumented
    // lookups recorded a span per call, which was most of a slow startup.
    let default_log_level = "warn";

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_log_level));

    // Write logs to stderr so they appear in VSCode's output channel
    let fmt_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false) // Disable colors for clean output in VSCode
        .with_target(false); // Hide target module paths for cleaner logs

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!("VSCode LSP server starting...");

    boot()
}
