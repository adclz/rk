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
        /// Output file path (default: <workspace>/rk_build/<profile>/core.wasm)
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
    #[command(display_order = 6)]
    Env {
        /// Print just these values, one per line, with no annotation
        /// (e.g. `rk env stdlib`)
        keys: Vec<String>,
    },
    /// Explain a diagnostic code (e.g. `rk explain E0301`)
    #[command(display_order = 5)]
    Explain {
        /// The code to explain, as diagnostics print it: E0301, L0002, ...
        code: String,
    },
    /// Format the workspace's .st files
    #[command(display_order = 4)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// The VSCode extension's command lines must parse. Read from the
    /// extension's source rather than restated, so a change to the argv in
    /// TypeScript is checked here.
    #[test]
    fn the_vscode_extension_invokes_a_cli_that_exists() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vscode/client/src/extension.ts"
        ))
        .expect("the extension source is part of this repo");

        // `rkCommand(), ["compile", "--workspace", ws]` — the argv arrays
        // handed to ProcessExecution and execFileSync.
        let invocations: Vec<Vec<String>> = source
            .match_indices("rkCommand(), [")
            .map(|(i, m)| {
                let rest = &source[i + m.len()..];
                let end = rest.find(']').expect("an argv array is closed");
                rest[..end]
                    .split(',')
                    .map(|a| a.trim().trim_matches('"').to_string())
                    .filter(|a| !a.is_empty())
                    .collect()
            })
            .collect();

        let mine: Vec<&Vec<String>> = invocations
            .iter()
            .filter(|argv| argv.first().map(String::as_str) != Some("debug"))
            .collect();
        assert_eq!(
            mine.len(),
            2,
            "expected the build task and the stdlib query; found {invocations:?}; \
             if the extension gained an invocation, cover it here too"
        );

        for argv in mine {
            // `ws` is the extension's variable for the workspace path.
            let argv: Vec<String> = std::iter::once("rk".to_string())
                .chain(argv.iter().map(|a| {
                    if a == "ws" {
                        ".".to_string()
                    } else {
                        a.clone()
                    }
                }))
                .collect();
            Args::try_parse_from(&argv).unwrap_or_else(|e| {
                panic!(
                    "the extension runs `{}`, which this CLI rejects:\n{e}",
                    argv.join(" ")
                )
            });
        }
    }
}
