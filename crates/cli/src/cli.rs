// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// How diagnostics are rendered: `full` is ariadne reports with color;
/// `concise` is one `FILE:LINE:COL: severity[CODE]: message` line per
/// diagnostic; `json-lines` one JSON object per line. Machine formats emit
/// no ANSI codes.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Ariadne reports with source excerpts and carets (human default)
    #[default]
    Full,
    /// One line per diagnostic: FILE:LINE:COL: severity[CODE]: message
    Concise,
    /// One JSON object per diagnostic per line (NDJSON)
    JsonLines,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "IEC 61131-3 Structured Text compiler")]
pub struct Args {
    /// The subcommand to run. With none, `rk` prints this help.
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub common: CommonArgs,
}

/// Options shared by every subcommand, marked `global` so they are
/// accepted at any position.
#[derive(clap::Args, Debug)]
pub struct CommonArgs {
    /// Path to the workspace (default: current directory)
    #[arg(long, global = true, default_value = ".")]
    pub workspace: PathBuf,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Diagnostics output format
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Full)]
    pub output_format: OutputFormat,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Check the workspace for diagnostics
    #[command(display_order = 1)]
    Check {
        /// Watch for file changes and re-run
        #[arg(short, long)]
        watch: bool,
    },
    /// Compile the workspace to WebAssembly
    #[command(display_order = 2)]
    Compile {
        /// Output file path (default: <profile>/<project>.wasm in the
        /// workspace's build folder, in the cache: `rk env build`)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Release profile: optimized (wasm-opt, mandatory), stepping tables
        /// and tests omitted. Monitoring and retained state still work;
        /// source-level stepping and `rk test` use the debug build.
        #[arg(long)]
        release: bool,

        /// Release optimization level: 0-4, s (size), z (aggressive size)
        #[arg(long, short = 'O', requires = "release")]
        opt_level: Option<String>,

        /// Deprecated alias for the default: the debug profile is what
        /// `rk compile` builds unless --release is given.
        #[arg(long, hide = true, conflicts_with = "release")]
        debug: bool,

        /// Watch for file changes and re-compile
        #[arg(short, long)]
        watch: bool,
    },
    /// Show the paths this toolchain resolved, and where each came from
    #[command(display_order = 7)]
    Env {
        /// Print just these values, one per line, with no annotation
        /// (e.g. `rk env stdlib`)
        keys: Vec<String>,
    },
    /// Explain a diagnostic code (e.g. `rk explain E0301`)
    #[command(display_order = 6)]
    Explain {
        /// The code to explain, as diagnostics print it: E0301, L0002, ...
        code: String,
    },
    /// Write the ST declarations of a library's component and of its host's interfaces
    ///
    /// In a library, by default: what the component in `component/` exports,
    /// and every interface of the package in `wit/`, or of each package in
    /// its `deps/` when there is none at its top. They go into `generated/`,
    /// one file per interface. A program names a WIT folder with `--wit`. A
    /// `.st` file this command wrote there before and no longer writes is
    /// removed; any other is left as it is.
    #[command(display_order = 9)]
    Bindgen {
        /// A WIT folder to read instead of the library's component and
        /// `wit/`: a host's package at its top, and in `deps/` the packages
        /// it uses, each a folder, a `.wit` file or a binary `.wasm` package
        #[arg(long, value_name = "FOLDER")]
        wit: Option<PathBuf>,

        /// Where the declarations go (default: <workspace>/generated)
        #[arg(short, long, value_name = "FOLDER")]
        output: Option<PathBuf>,

        /// The world whose imports are written, when the folder's package
        /// declares several
        #[arg(long)]
        world: Option<String>,

        /// Declarations every program reaches; by default they are
        /// INTERNAL, reached by the library that holds them only
        #[arg(long)]
        public: bool,
    },
    /// Fetch the libraries the workspace depends on, or add one
    #[command(display_order = 8)]
    Fetch {
        /// Fetch the library at this URL, check it, and add it to
        /// config.toml with the hash of what it holds
        #[arg(value_name = "URL")]
        url: Option<String>,
    },
    /// Delete the workspace's builds from the cache
    ///
    /// What `rk compile` and `rk test` wrote for the workspace, in its build
    /// folder in the cache (`rk env build`). The libraries the cache holds
    /// stay, and so do the other workspaces' builds.
    #[command(display_order = 4)]
    Clean,
    /// Format the workspace's .st files
    #[command(display_order = 5)]
    Fmt {
        /// Check formatting without writing (exit 1 if unformatted)
        #[arg(long)]
        check: bool,
    },
    /// Compile the workspace and run its tests
    #[command(display_order = 3)]
    Test {
        /// Run a specific test by name (substring match)
        #[arg()]
        test_name: Option<String>,

        /// Optimization level: 0-4, s (size), z (aggressive size)
        #[arg(long, short = 'O')]
        opt_level: Option<String>,

        /// How long one test may run before it is stopped, e.g. `30s`, `500ms`
        ///
        /// A test that waits out a timer's preset legitimately runs long; a
        /// test that never returns must still not hang the run.
        #[arg(long)]
        timeout: Option<String>,
    },
}
