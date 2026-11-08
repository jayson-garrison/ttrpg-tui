//! Oraculum core: pure logic with no terminal or file I/O.
//!
//! All randomness goes through [`rng::Roller`]; all time through [`clock::Clock`].

pub mod catalog;
pub mod clock;
pub mod dice;
pub mod error;
pub mod generator;
pub mod oracle;
pub mod rng;
pub mod table;
pub mod template;
