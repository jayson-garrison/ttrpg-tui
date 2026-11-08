//! Oraculum core: pure logic with no terminal or file I/O.
//!
//! All randomness goes through [`rng::Roller`]; all time through [`clock::Clock`].

pub mod builtin;
pub mod catalog;
pub mod clock;
pub mod dice;
pub mod engine;
pub mod error;
pub mod generator;
pub mod journal;
pub mod oracle;
pub mod rng;
pub mod table;
pub mod template;
