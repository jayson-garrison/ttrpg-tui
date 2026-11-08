//! Errors raised while rolling already-validated content.

use thiserror::Error;

/// Rolling errors. For catalog-validated content these cannot occur except
/// through the defensive depth guard.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvalError {
    #[error("unknown table {0:?}")]
    UnknownTable(String),
    #[error("unknown generator {0:?}")]
    UnknownGenerator(String),
    #[error("references nested deeper than {max} levels")]
    DepthExceeded { max: usize },
    #[error("table {table:?} has no entry for total {total}")]
    NoEntry { table: String, total: i64 },
}

/// Maximum nesting of `{table:..}` references (D-16).
pub const MAX_DEPTH: usize = 5;
