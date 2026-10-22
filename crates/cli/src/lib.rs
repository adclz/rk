// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub mod cli;
pub mod commands;
pub mod compiler;
pub mod diagnostics;
pub mod duration;
pub mod error;
pub mod file_order;
pub mod spawn;
pub mod test_host;
pub mod test_runner;
pub mod ui;
pub mod wasm_opt;
pub mod watcher;
pub mod workspace;

use std::path::Path;

use cli::{Command, OutputFormat};
use error::CliResult;

/// What the `rk` binary does: the process's arguments read, the command
/// run, and its outcome as the exit code.
///
/// A tool built on rk is this after it added its front-end
/// (`hir::front_end::register`): rk, reading its files as well.
///
/// ```ignore
/// fn main() -> std::process::ExitCode {
///     other::register();
///     rk::main()
/// }
/// ```
pub fn main() -> std::process::ExitCode {
    restore_sigpipe();
    exit_code(run_args(std::env::args_os()))
}

/// [`main`] on arguments given: the program's name first, as a process
/// receives them. Arguments that name no command print the help, and ones
/// that cannot be read end the process the way they do for `rk`.
pub fn run_args<I, T>(args: I) -> CliResult<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::{CommandFactory, Parser};

    let args = cli::Args::parse_from(args);
    let workspace = args.common.workspace.as_path();

    // One process-wide color decision, before any output.
    ui::init_output(args.common.output_format);

    // The default is the current directory, which exists; only an explicit
    // --workspace can name nothing, and every command reads it.
    workspace::require_workspace_dir(workspace)?;

    match args.command {
        Some(command) => run(
            workspace,
            args.common.verbose,
            args.common.output_format,
            command,
        ),
        None => {
            cli::Args::command()
                .print_help()
                .map_err(error::CliError::msg)?;
            println!();
            Ok(())
        }
    }
}

/// Run one compiler command against `workspace`: what [`run_args`] does
/// once the arguments are read, for a tool that reads its own.
pub fn run(
    workspace: &Path,
    verbose: bool,
    format: OutputFormat,
    command: Command,
) -> CliResult<()> {
    use commands::{check, compile, env, explain, fetch, fmt, test};
    match command {
        Command::Check { watch } => check::run_check(workspace, watch, verbose, format),
        Command::Env { keys } => env::run_env(workspace, &keys, format),
        Command::Explain { code } => explain::run_explain(&code, format),
        Command::Fetch { save } => fetch::run_fetch(workspace, save.as_deref(), format),
        Command::Compile {
            output,
            release,
            opt_level,
            debug: _, // deprecated alias for the default profile
            watch,
        } => compile::run_compile(
            workspace,
            compile::CompileOptions {
                output: output.as_ref(),
                opt_level: opt_level.as_deref(),
                release,
                format,
            },
            watch,
            verbose,
        ),
        Command::Fmt { check } => fmt::run_fmt(workspace, check, verbose, format),
        Command::Test {
            test_name,
            opt_level,
            timeout,
        } => test::run_test(
            workspace,
            test_name.as_deref(),
            opt_level.as_deref(),
            timeout.as_deref(),
            verbose,
            format,
        ),
    }
}

/// Rust ignores SIGPIPE at startup, so a reader that closes early
/// (`rk check | head`) turned every later write into a panic; restore the
/// default so the process dies quietly.
pub fn restore_sigpipe() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// The process exit code for a command's outcome: a message is printed here,
/// a plain failure was already explained by the command.
pub fn exit_code(result: CliResult<()>) -> std::process::ExitCode {
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error::CliError::Message(msg)) => {
            ui::error(msg);
            std::process::ExitCode::FAILURE
        }
        Err(error::CliError::Failed) => std::process::ExitCode::FAILURE,
    }
}
