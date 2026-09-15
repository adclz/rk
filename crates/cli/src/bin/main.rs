use std::process::ExitCode;

use rk::cli::Args;
use rk::error::CliResult;
use clap::{CommandFactory, Parser};

fn main() -> ExitCode {
    rk::restore_sigpipe();
    rk::exit_code(run())
}

fn run() -> CliResult<()> {
    let args = Args::parse();
    let workspace = args.common.workspace.as_path();

    // One process-wide color decision, before any output.
    rk::ui::init_output(args.common.output_format);

    // The default is the current directory, which exists; only an explicit
    // --workspace can name nothing, and every command reads it.
    rk::workspace::require_workspace_dir(workspace)?;

    match args.command {
        Some(command) => rk::run(
            workspace,
            args.common.verbose,
            args.common.output_format,
            command,
        ),
        None => {
            Args::command()
                .print_help()
                .map_err(rk::error::CliError::msg)?;
            println!();
            Ok(())
        }
    }
}
