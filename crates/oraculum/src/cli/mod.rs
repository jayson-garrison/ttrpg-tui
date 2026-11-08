//! Command line interface (spec 8; A§5.2).
//!
//! `run` takes writers so tests can call it in-process. Every roll goes
//! through `Engine`, so stdout is byte-identical to the journal's `result`.

mod output;

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use clap::{Parser, Subcommand, ValueEnum};
use oraculum_core::catalog::{Catalog, Filter, ItemKind, Source};
use oraculum_core::clock::Clock;
use oraculum_core::engine::{Engine, Rolled};
use oraculum_core::error::EvalError;
use oraculum_core::journal::export_markdown;
use oraculum_core::oracle::Likelihood;
use oraculum_core::rng::RngRoller;

use crate::error::AppError;
use crate::storage::{load_user_sources, write_export, DataDir, JournalStore, SessionName};

#[derive(Debug, Parser)]
#[command(
    name = "oraculum",
    version,
    about = "Oracle, dice and tables for solo and GMless tabletop RPG play"
)]
pub struct Cli {
    /// Where tables, generators and sessions live (default: the platform data directory).
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
    /// Journal session name.
    #[arg(long, global = true, default_value = "default")]
    pub session: String,
    /// Make every roll reproducible.
    #[arg(long, global = true)]
    pub seed: Option<u64>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LikelihoodArg {
    VeryUnlikely,
    Unlikely,
    Even,
    Likely,
    VeryLikely,
}

impl From<LikelihoodArg> for Likelihood {
    fn from(a: LikelihoodArg) -> Self {
        match a {
            LikelihoodArg::VeryUnlikely => Likelihood::VeryUnlikely,
            LikelihoodArg::Unlikely => Likelihood::Unlikely,
            LikelihoodArg::Even => Likelihood::Even,
            LikelihoodArg::Likely => Likelihood::Likely,
            LikelihoodArg::VeryLikely => Likelihood::VeryLikely,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Roll a dice expression, e.g. "3d6+2" or "2d20kl1".
    Roll { expr: String },
    /// Ask the oracle a yes/no question.
    Ask {
        likelihood: LikelihoodArg,
        question: Option<String>,
    },
    /// Roll a table by id.
    Table { id: String },
    /// Run a generator by id.
    Gen { id: String },
    /// List tables and generators.
    List {
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        search: Option<String>,
    },
    /// Write the session journal as Markdown.
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

pub struct Env<'a> {
    pub data_dir: DataDir,
    pub session: SessionName,
    pub seed: u64,
    pub clock: Box<dyn Clock>,
    /// Built-in content; tests pass fixtures so they never pin shipped text (D-72).
    pub builtin: Vec<Source>,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

pub fn run(cmd: Command, env: Env<'_>) -> Result<(), AppError> {
    let Env {
        data_dir,
        session,
        seed,
        clock,
        builtin,
        out,
        err,
    } = env;
    let store = JournalStore::new(data_dir.session_path(&session));

    match cmd {
        Command::Roll { expr } => {
            // Dice need no content, so none is loaded and no warnings print (D-29).
            let mut engine = engine(Rc::new(Catalog::default()), seed, clock);
            let rolled = engine
                .roll_dice(&expr)
                .map_err(|e| AppError::Input(format!("bad dice expression: {e}")))?;
            finish(&rolled, out, &store)
        }
        Command::Ask {
            likelihood,
            question,
        } => {
            let mut engine = engine(load_catalog(&data_dir, &builtin, err)?, seed, clock);
            let rolled = engine.ask(likelihood.into(), question.as_deref());
            finish(&rolled, out, &store)
        }
        Command::Table { id } => {
            let catalog = load_catalog(&data_dir, &builtin, err)?;
            let mut engine = engine(Rc::clone(&catalog), seed, clock);
            let rolled = engine
                .roll_table(&id)
                .map_err(|e| eval_error(&e, &catalog))?;
            finish(&rolled, out, &store)
        }
        Command::Gen { id } => {
            let catalog = load_catalog(&data_dir, &builtin, err)?;
            let mut engine = engine(Rc::clone(&catalog), seed, clock);
            let rolled = engine.generate(&id).map_err(|e| eval_error(&e, &catalog))?;
            finish(&rolled, out, &store)
        }
        Command::List { category, search } => {
            let catalog = load_catalog(&data_dir, &builtin, err)?;
            let filter = Filter {
                text: search,
                category,
                kind: None,
            };
            out.write_all(output::format_list(&catalog.search(&filter)).as_bytes())?;
            Ok(())
        }
        Command::Export { out: target } => {
            let load = store.load()?;
            for s in &load.skipped {
                writeln!(
                    err,
                    "{}",
                    output::format_skipped_line(store.path(), s.line_no, &s.reason)
                )?;
            }
            let path = target.unwrap_or_else(|| data_dir.export_path());
            write_export(&path, &export_markdown(session.as_str(), &load.entries))?;
            writeln!(out, "{}", path.display())?;
            Ok(())
        }
    }
}

fn engine(catalog: Rc<Catalog>, seed: u64, clock: Box<dyn Clock>) -> Engine {
    Engine::new(catalog, Box::new(RngRoller::from_seed(seed)), clock)
}

/// Built-in plus user content; every load warning goes to stderr (D-60).
fn load_catalog(
    data_dir: &DataDir,
    builtin: &[Source],
    err: &mut dyn Write,
) -> Result<Rc<Catalog>, AppError> {
    let (user, file_warnings) = load_user_sources(data_dir);
    let (catalog, warnings) = Catalog::build(builtin, &user);
    for w in file_warnings.iter().chain(&warnings) {
        writeln!(err, "{}", output::format_warning(w))?;
    }
    Ok(Rc::new(catalog))
}

/// Print the result, then journal it. A journal failure is reported after the
/// result has been shown (D-52).
fn finish(rolled: &Rolled, out: &mut dyn Write, store: &JournalStore) -> Result<(), AppError> {
    writeln!(out, "{}", rolled.entry.result)?;
    store.append(&rolled.entry)?;
    Ok(())
}

fn eval_error(e: &EvalError, catalog: &Catalog) -> AppError {
    let (kind, id) = match e {
        EvalError::UnknownTable(id) => (ItemKind::Table, id),
        EvalError::UnknownGenerator(id) => (ItemKind::Generator, id),
        other => return AppError::Input(other.to_string()),
    };
    let ids = catalog
        .items()
        .iter()
        .filter(|i| i.kind == kind)
        .map(|i| i.id.as_str());
    let close = output::suggestions(id, ids);
    AppError::Input(output::unknown_id_message(kind, id, &close))
}
