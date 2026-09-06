//! NYEDArch builder library.
//!
//! The sealing pipeline lives here rather than inside the command-line binary,
//! so the desktop client and the CLI run the *same* code. There is deliberately
//! no second implementation for the GUI to call: an interface that reports
//! progress must be reporting real work.

pub mod artifact;
pub mod cli;
pub mod config;
pub mod eula;
pub mod generator;
pub mod harden;
pub mod keystore;
pub mod machines;
pub mod pipeline;
pub mod remote;
pub mod srcpack;

pub use pipeline::{seal_and_generate, runtime_source_root, SealError, SealOutcome, SealRequest, Stage};
