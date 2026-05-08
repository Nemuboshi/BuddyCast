//! Core library for the `buddy_cast` command line tool.
//!
//! The crate is intentionally split into small modules so that the startup
//! binary remains thin and the business logic stays testable.

pub mod api;
pub mod archive;
pub mod cli;
pub mod db;
pub mod decrypt;
pub mod error;
pub mod model;
pub mod progress;
pub mod subtitle;
pub mod workflow;

use anyhow::Result;

/// Run the command line application and convert all internal errors into an
/// application-level result.
pub fn run() -> Result<()> {
    cli::run()
}
