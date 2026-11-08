//! Oraculum entry point. The only place `anyhow` is used (A§5.1).

use std::io::Write;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use oraculum_app::cli::{run, Cli, Env};
use oraculum_app::clock::SystemClock;
use oraculum_app::error::AppError;
use oraculum_app::storage::{DataDir, SessionName};

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(e) => {
            // Nothing sensible is left to do if stderr itself is broken.
            let _ = writeln!(std::io::stderr(), "oraculum: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn real_main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        let _ = writeln!(
            std::io::stderr(),
            "oraculum: the interactive interface is not available in this build yet; \
             use a subcommand (see --help)"
        );
        return Ok(ExitCode::from(2));
    };

    let outcome = (|| -> Result<(), AppError> {
        let session = SessionName::parse(&cli.session)?;
        let data_dir = DataDir::resolve(cli.data_dir.clone())?;
        let seed = cli.seed.unwrap_or_else(rand::random);
        let mut stdout = std::io::stdout().lock();
        let mut stderr = std::io::stderr().lock();
        let env = Env {
            builtin: oraculum_core::builtin::sources(),
            data_dir,
            session,
            seed,
            clock: Box::new(SystemClock),
            out: &mut stdout,
            err: &mut stderr,
        };
        run(command, env)
    })();

    match outcome {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(e) => {
            writeln!(std::io::stderr(), "oraculum: {e}").context("cannot write to stderr")?;
            Ok(ExitCode::from(e.exit_code()))
        }
    }
}
