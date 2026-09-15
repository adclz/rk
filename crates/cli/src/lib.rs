pub mod cli;
pub mod commands;
pub mod compiler;
pub mod diagnostics;
pub mod duration;
pub mod error;
pub mod file_order;
pub mod reports;
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

/// Run one compiler command against `workspace`: the binary's dispatcher,
/// kept in the library so a wrapping tool runs the shared commands the
/// same way.
pub fn run(
    workspace: &Path,
    verbose: bool,
    format: OutputFormat,
    command: Command,
) -> CliResult<()> {
    use commands::{check, compile, env, explain, fmt, test};
    match command {
        Command::Check { watch } => check::run_check(workspace, watch, verbose, format),
        Command::Env { keys } => env::run_env(workspace, &keys, format),
        Command::Explain { code } => explain::run_explain(&code, format),
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
