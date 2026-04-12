//! convergio-autoresearch — nightly optimization loop.
//!
//! Scheduled task (02:00): collect metrics, propose optimizations via local
//! MLX model, test in temporary worktree, record experiment results.
//! All experiments are tracked in DB — nothing runs unless measured.
//!
//! DB tables: autoresearch_experiments, autoresearch_metrics.

pub mod ext;
pub mod metrics;
pub mod routes;
pub mod runner;
pub mod types;

pub use ext::AutoresearchExtension;
pub use types::{Experiment, ExperimentOutcome, ExperimentStatus};

pub mod mcp_defs;
#[cfg(test)]
mod tests;
