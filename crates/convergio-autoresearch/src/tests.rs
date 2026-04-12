//! Tests for convergio-autoresearch.

mod ext_tests {
    use convergio_types::extension::Extension;
    use convergio_types::manifest::ModuleKind;

    use crate::ext::AutoresearchExtension;

    #[test]
    fn manifest_is_extension_kind() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = AutoresearchExtension::new(pool);
        let m = ext.manifest();
        assert_eq!(m.id, "convergio-autoresearch");
        assert!(matches!(m.kind, ModuleKind::Extension));
        assert!(m.provides.iter().any(|c| c.name == "autoresearch"));
        assert!(m.provides.iter().any(|c| c.name == "metrics-collection"));
    }

    #[test]
    fn has_one_migration() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = AutoresearchExtension::new(pool);
        let migs = ext.migrations();
        assert_eq!(migs.len(), 1);
    }

    #[test]
    fn migrations_sql_valid() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let ext = AutoresearchExtension::new(pool.clone());
        for mig in ext.migrations() {
            conn.execute_batch(mig.up).unwrap();
        }
        // Verify tables exist
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM autoresearch_experiments", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM autoresearch_metrics", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn has_scheduled_task() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = AutoresearchExtension::new(pool);
        let tasks = ext.scheduled_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "autoresearch-nightly");
        assert_eq!(tasks[0].cron, "0 2 * * *");
    }

    #[test]
    fn health_ok_with_pool() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = AutoresearchExtension::new(pool);
        assert!(matches!(
            ext.health(),
            convergio_types::extension::Health::Ok
        ));
    }
}

mod types_tests {
    use crate::types::*;

    #[test]
    fn experiment_status_display() {
        assert_eq!(ExperimentStatus::Pending.to_string(), "pending");
        assert_eq!(ExperimentStatus::Running.to_string(), "running");
        assert_eq!(ExperimentStatus::Completed.to_string(), "completed");
        assert_eq!(ExperimentStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn experiment_outcome_display() {
        assert_eq!(ExperimentOutcome::Pending.to_string(), "pending");
        assert_eq!(ExperimentOutcome::Kept.to_string(), "kept");
        assert_eq!(ExperimentOutcome::Discarded.to_string(), "discarded");
        assert_eq!(ExperimentOutcome::Error.to_string(), "error");
    }

    #[test]
    fn default_config() {
        let cfg = AutoresearchConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.cron, "0 2 * * *");
        assert!(cfg.model.contains("Qwen"));
        assert_eq!(cfg.max_experiments_per_night, 3);
    }
}

mod runner_tests {
    use crate::runner::{safe_truncate, validate_daemon_url};

    #[test]
    fn create_experiment_with_valid_pool() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let ext = crate::ext::AutoresearchExtension::new(pool.clone());
        for mig in convergio_types::extension::Extension::migrations(&ext) {
            conn.execute_batch(mig.up).unwrap();
        }
        conn.execute(
            "INSERT INTO autoresearch_experiments \
             (target_file, description, status, outcome, model_used, proposal) \
             VALUES ('test.rs', 'test', 'pending', 'pending', 'test-model', 'proposal')",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM autoresearch_experiments", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn validate_daemon_url_allows_localhost() {
        assert!(validate_daemon_url("http://localhost:8420").is_ok());
        assert!(validate_daemon_url("http://127.0.0.1:8420").is_ok());
        assert!(validate_daemon_url("https://localhost:8420").is_ok());
    }

    #[test]
    fn validate_daemon_url_rejects_external() {
        assert!(validate_daemon_url("http://evil.com:8420").is_err());
        assert!(validate_daemon_url("http://192.168.1.1:8420").is_err());
        assert!(validate_daemon_url("ftp://localhost:8420").is_err());
        assert!(validate_daemon_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn safe_truncate_ascii() {
        assert_eq!(safe_truncate("hello world", 5), "hello");
        assert_eq!(safe_truncate("short", 100), "short");
    }

    #[test]
    fn safe_truncate_multibyte() {
        // "café" is 5 bytes (é = 2 bytes)
        let s = "café";
        assert_eq!(s.len(), 5);
        // Truncating at 4 would split é — should back up to 3
        let t = safe_truncate(s, 4);
        assert_eq!(t, "caf");
        // Truncating at 5 keeps the full string
        assert_eq!(safe_truncate(s, 5), "café");
    }
}

mod metrics_tests {
    #[test]
    fn insert_metrics_with_valid_pool() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let ext = crate::ext::AutoresearchExtension::new(pool.clone());
        for mig in convergio_types::extension::Extension::migrations(&ext) {
            conn.execute_batch(mig.up).unwrap();
        }
        conn.execute(
            "INSERT INTO autoresearch_metrics \
             (test_count, test_duration_secs, binary_size_bytes, \
              total_rust_lines, crate_count) VALUES (100, 5.5, 1000000, 50000, 28)",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM autoresearch_metrics", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
}
