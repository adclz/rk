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
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Initialize tracing based on build type and environment
    #[cfg(debug_assertions)]
    let default_log_level = "debug";
    #[cfg(not(debug_assertions))]
    let default_log_level = "off";

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_log_level));

    let subscriber = Registry::default()
        .with(env_filter)
        .with(tracing_span_tree::SpanTree::default());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    tracing::info!("VSCode LSP server starting...");

    boot()
}
