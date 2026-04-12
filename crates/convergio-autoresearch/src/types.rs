//! Core types for the autoresearch loop.

use serde::{Deserialize, Serialize};

/// A single optimization experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: i64,
    pub target_file: String,
    pub description: String,
    pub status: ExperimentStatus,
    pub outcome: ExperimentOutcome,
    pub baseline_test_secs: Option<f64>,
    pub experiment_test_secs: Option<f64>,
    pub binary_size_before: Option<u64>,
    pub binary_size_after: Option<u64>,
    pub model_used: String,
    pub proposal: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Experiment lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for ExperimentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Experiment result: kept, discarded, or errored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentOutcome {
    Pending,
    Kept,
    Discarded,
    Error,
}

impl std::fmt::Display for ExperimentOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Kept => write!(f, "kept"),
            Self::Discarded => write!(f, "discarded"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Snapshot of project metrics collected before each experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetrics {
    pub test_count: u32,
    pub test_duration_secs: f64,
    pub binary_size_bytes: u64,
    pub total_rust_lines: u64,
    pub crate_count: u32,
    pub collected_at: String,
}

/// Configuration for the autoresearch loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoresearchConfig {
    pub enabled: bool,
    pub cron: String,
    pub daemon_url: String,
    pub model: String,
    pub max_experiments_per_night: u32,
    pub repo_root: String,
}

impl Default for AutoresearchConfig {
    fn default() -> Self {
        let repo_root = std::env::var("CONVERGIO_REPO_ROOT")
            .or_else(|_| std::env::current_dir().map(|p| p.display().to_string()))
            .unwrap_or_else(|_| ".".into());
        Self {
            enabled: true,
            cron: "0 2 * * *".into(),
            daemon_url: "http://localhost:8420".into(),
            model: "mlx-community/Qwen2.5-Coder-7B-Instruct-4bit".into(),
            max_experiments_per_night: 3,
            repo_root,
        }
    }
}
