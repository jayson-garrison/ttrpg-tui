//! Oraculum entry point. The only place `anyhow` is used (A§5.1).

use std::io::Write;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use oraculum_app::cli::{run, Cli, Env};
use oraculum_app::clock::SystemClock;
use oraculum_app::error::AppError;
use oraculum_app::storage::{DataDir, SessionName};
use oraculum_app::tui::action::AppInfo;
use oraculum_app::tui::App;

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
        return run_tui(&cli);
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

/// No subcommand: open the interface (spec 4).
fn run_tui(cli: &Cli) -> anyhow::Result<ExitCode> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return fail(&AppError::Input(
            "the interactive interface needs a terminal; use a subcommand when piping (see --help)"
                .into(),
        ));
    }
    let session = match SessionName::parse(&cli.session) {
        Ok(s) => s,
        Err(e) => return fail(&AppError::from(e)),
    };
    let data_dir = match DataDir::resolve(cli.data_dir.clone()) {
        Ok(d) => d,
        Err(e) => return fail(&AppError::from(e)),
    };
    let seed = cli.seed.unwrap_or_else(rand::random);
    let (_catalog, warnings) =
        oraculum_app::cli::build_catalog(&data_dir, &oraculum_core::builtin::sources());
    let info = AppInfo {
        seed,
        session: session.as_str().to_string(),
        warnings,
        data_dir,
    };
    let app = App::new(info, Box::new(SystemClock));
    oraculum_app::tui::run(app).context("terminal error")?;
    Ok(ExitCode::SUCCESS)
}

fn fail(e: &AppError) -> anyhow::Result<ExitCode> {
    writeln!(std::io::stderr(), "oraculum: {e}").context("cannot write to stderr")?;
    Ok(ExitCode::from(e.exit_code()))
}
