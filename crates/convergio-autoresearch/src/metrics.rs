//! Project metrics collection — objective measurements before/after experiments.
//!
//! Rust projects: cargo test time, binary size, line count, crate count.

use crate::types::ProjectMetrics;
use std::path::Path;
use std::time::Instant;

/// Collect project metrics for a Rust workspace.
pub fn collect_rust_metrics(repo_root: &Path) -> ProjectMetrics {
    let daemon_dir = repo_root.join("daemon");
    let test_duration = measure_test_duration(&daemon_dir);
    let test_count = count_tests(&daemon_dir);
    let binary_size = binary_size_bytes(&daemon_dir);
    let total_lines = count_rust_lines(repo_root);
    let crate_count = count_crates(&daemon_dir);

    ProjectMetrics {
        test_count,
        test_duration_secs: test_duration,
        binary_size_bytes: binary_size,
        total_rust_lines: total_lines,
        crate_count,
        collected_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn measure_test_duration(daemon_dir: &Path) -> f64 {
    let start = Instant::now();
    let status = std::process::Command::new("cargo")
        .args(["test", "--workspace", "--quiet"])
        .current_dir(daemon_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => start.elapsed().as_secs_f64(),
        _ => -1.0,
    }
}

fn count_tests(daemon_dir: &Path) -> u32 {
    let output = std::process::Command::new("cargo")
        .args(["test", "--workspace", "--", "--list"])
        .current_dir(daemon_dir)
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.ends_with(": test"))
            .count() as u32,
        Err(_) => 0,
    }
}

fn binary_size_bytes(daemon_dir: &Path) -> u64 {
    let binary = daemon_dir.join("target/release/convergio");
    std::fs::metadata(&binary).map(|m| m.len()).unwrap_or(0)
}

fn count_rust_lines(repo_root: &Path) -> u64 {
    let output = std::process::Command::new("find")
        .args([
            repo_root.to_str().unwrap_or("."),
            "-name",
            "*.rs",
            "-not",
            "-path",
            "*/target/*",
        ])
        .output();
    match output {
        Ok(o) => {
            let files: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect();
            let mut total: u64 = 0;
            for f in &files {
                if let Ok(content) = std::fs::read_to_string(f) {
                    total += content.lines().count() as u64;
                }
            }
            total
        }
        Err(_) => 0,
    }
}

fn count_crates(daemon_dir: &Path) -> u32 {
    let crates_dir = daemon_dir.join("crates");
    std::fs::read_dir(&crates_dir)
        .map(|entries| entries.filter_map(|e| e.ok()).count() as u32)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn binary_size_returns_zero_for_missing() {
        assert_eq!(binary_size_bytes(&PathBuf::from("/nonexistent")), 0);
    }

    #[test]
    fn count_crates_returns_zero_for_missing() {
        assert_eq!(count_crates(&PathBuf::from("/nonexistent")), 0);
    }

    #[test]
    fn count_rust_lines_returns_zero_for_missing() {
        assert_eq!(count_rust_lines(&PathBuf::from("/nonexistent")), 0);
    }
}
