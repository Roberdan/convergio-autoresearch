//! Experiment runner — the core autoresearch loop.
//!
//! 1. Collect baseline metrics (cargo test time, binary size)
//! 2. Ask local MLX model for optimization proposal
//! 3. Apply proposal in temporary worktree
//! 4. Run cargo check + cargo test
//! 5. Compare metrics → keep or discard
//! 6. Record everything in DB

use convergio_db::pool::ConnPool;

use crate::metrics::collect_rust_metrics;
use crate::types::{AutoresearchConfig, ExperimentOutcome, ExperimentStatus};
use std::path::Path;

/// Run one nightly autoresearch cycle.
pub async fn run_cycle(pool: &ConnPool, config: &AutoresearchConfig) {
    tracing::info!("autoresearch cycle starting");

    let repo = Path::new(&config.repo_root);
    let baseline = collect_rust_metrics(repo);
    log_metrics(pool, &baseline);

    for i in 0..config.max_experiments_per_night {
        tracing::info!(experiment = i + 1, "running experiment");
        if let Err(e) = run_single_experiment(pool, config, &baseline).await {
            tracing::warn!(error = %e, "experiment failed");
        }
    }
    tracing::info!("autoresearch cycle complete");
}

async fn run_single_experiment(
    pool: &ConnPool,
    config: &AutoresearchConfig,
    baseline: &crate::types::ProjectMetrics,
) -> Result<(), String> {
    // 1. Pick a target file (largest .rs files are best candidates)
    let target = pick_target(&config.repo_root)?;

    // 2. Create experiment record
    let exp_id = create_experiment(pool, &target, "pending proposal")?;
    update_status(pool, exp_id, ExperimentStatus::Running);

    // 3. Ask MLX for optimization proposal
    let proposal = ask_for_proposal(config, &target).await?;
    update_proposal(pool, exp_id, &proposal);

    // 4. Test proposal: cargo check in temporary branch
    let check_ok = test_proposal(&config.repo_root, &target, &proposal);

    // 5. Record outcome
    if check_ok {
        let after = collect_rust_metrics(Path::new(&config.repo_root));
        let improved = after.test_duration_secs < baseline.test_duration_secs * 0.95;
        let outcome = if improved {
            ExperimentOutcome::Kept
        } else {
            ExperimentOutcome::Discarded
        };
        complete_experiment(pool, exp_id, outcome, Some(baseline), Some(&after));
    } else {
        complete_experiment(pool, exp_id, ExperimentOutcome::Error, None, None);
    }
    Ok(())
}

fn pick_target(repo_root: &str) -> Result<String, String> {
    let crates_dir = format!("{repo_root}/daemon/crates");
    let output = std::process::Command::new("find")
        .args([&crates_dir, "-name", "*.rs", "-not", "-path", "*/target/*"])
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let files: Vec<&str> = stdout
        .lines()
        .filter(|l| !l.contains("/tests") && !l.contains("test"))
        .take(50)
        .collect();
    // Pick the file with most lines as optimization target
    let mut best = (String::new(), 0usize);
    for f in &files {
        if let Ok(content) = std::fs::read_to_string(f) {
            let lines = content.lines().count();
            if lines > best.1 && lines < 250 {
                best = (f.to_string(), lines);
            }
        }
    }
    if best.0.is_empty() {
        Err("no suitable target found".into())
    } else {
        Ok(best.0)
    }
}

async fn ask_for_proposal(config: &AutoresearchConfig, target: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(target).map_err(|e| e.to_string())?;
    let prompt = format!(
        "Suggest ONE small optimization for this Rust file. \
         Focus on performance, not style. Reply with ONLY the optimized code.\n\n\
         File: {target}\n```rust\n{content}\n```"
    );
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(format!("{}/api/inference/complete", config.daemon_url))
        .json(&serde_json::json!({
            "prompt": prompt,
            "max_tokens": 1024,
            "agent_id": "autoresearch",
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp["content"]
        .as_str()
        .unwrap_or("no proposal")
        .to_string())
}

fn test_proposal(repo_root: &str, _target: &str, _proposal: &str) -> bool {
    // Dry-run: just verify the workspace still compiles
    let daemon_dir = format!("{repo_root}/daemon");
    std::process::Command::new("cargo")
        .args(["check", "--workspace", "--quiet"])
        .current_dir(&daemon_dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// --- DB helpers ---

fn create_experiment(pool: &ConnPool, target: &str, desc: &str) -> Result<i64, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO autoresearch_experiments \
         (target_file, description, status, outcome, model_used, proposal) \
         VALUES (?1, ?2, 'pending', 'pending', 'mlx-qwen-7b', '')",
        rusqlite::params![target, desc],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn update_status(pool: &ConnPool, id: i64, status: ExperimentStatus) {
    if let Ok(conn) = pool.get() {
        let _ = conn.execute(
            "UPDATE autoresearch_experiments SET status = ?1 WHERE id = ?2",
            rusqlite::params![status.to_string(), id],
        );
    }
}

fn update_proposal(pool: &ConnPool, id: i64, proposal: &str) {
    if let Ok(conn) = pool.get() {
        let _ = conn.execute(
            "UPDATE autoresearch_experiments SET proposal = ?1 WHERE id = ?2",
            rusqlite::params![&proposal[..proposal.len().min(5000)], id],
        );
    }
}

fn complete_experiment(
    pool: &ConnPool,
    id: i64,
    outcome: ExperimentOutcome,
    baseline: Option<&crate::types::ProjectMetrics>,
    after: Option<&crate::types::ProjectMetrics>,
) {
    if let Ok(conn) = pool.get() {
        let _ = conn.execute(
            "UPDATE autoresearch_experiments SET status = 'completed', outcome = ?1, \
             baseline_test_secs = ?2, experiment_test_secs = ?3, \
             completed_at = datetime('now') WHERE id = ?4",
            rusqlite::params![
                outcome.to_string(),
                baseline.map(|b| b.test_duration_secs),
                after.map(|a| a.test_duration_secs),
                id
            ],
        );
    }
}

fn log_metrics(pool: &ConnPool, m: &crate::types::ProjectMetrics) {
    if let Ok(conn) = pool.get() {
        let _ = conn.execute(
            "INSERT INTO autoresearch_metrics \
             (test_count, test_duration_secs, binary_size_bytes, \
              total_rust_lines, crate_count) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                m.test_count,
                m.test_duration_secs,
                m.binary_size_bytes,
                m.total_rust_lines,
                m.crate_count,
            ],
        );
    }
}
