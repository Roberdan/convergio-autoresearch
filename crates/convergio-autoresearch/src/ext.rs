//! Extension trait implementation for convergio-autoresearch.

use std::sync::Arc;

use convergio_db::pool::ConnPool;
use convergio_types::extension::{
    AppContext, Extension, Health, McpToolDef, Metric, Migration, ScheduledTask,
};
use convergio_types::manifest::{Capability, Manifest, ModuleKind};

use crate::routes::{autoresearch_routes, AutoresearchState};
use crate::types::AutoresearchConfig;

pub struct AutoresearchExtension {
    pool: ConnPool,
    config: AutoresearchConfig,
}

impl AutoresearchExtension {
    pub fn new(pool: ConnPool) -> Self {
        Self {
            pool,
            config: AutoresearchConfig::default(),
        }
    }

    fn state(&self) -> Arc<AutoresearchState> {
        Arc::new(AutoresearchState {
            pool: self.pool.clone(),
            config: self.config.clone(),
        })
    }
}

impl Extension for AutoresearchExtension {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "convergio-autoresearch".to_string(),
            description: "Nightly optimization loop — experiment, measure, keep or discard"
                .to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            kind: ModuleKind::Extension,
            provides: vec![
                Capability {
                    name: "autoresearch".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Nightly code optimization experiments".to_string(),
                },
                Capability {
                    name: "metrics-collection".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Project metrics (test time, binary size, lines)".to_string(),
                },
            ],
            requires: vec![],
            agent_tools: vec![],
            required_roles: vec!["orchestrator".into(), "all".into()],
        }
    }

    fn routes(&self, _ctx: &AppContext) -> Option<axum::Router> {
        Some(autoresearch_routes(self.state()))
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            description: "autoresearch tables",
            up: "CREATE TABLE IF NOT EXISTS autoresearch_experiments (\
                    id INTEGER PRIMARY KEY,\
                    target_file TEXT NOT NULL,\
                    description TEXT,\
                    status TEXT DEFAULT 'pending',\
                    outcome TEXT DEFAULT 'pending',\
                    baseline_test_secs REAL,\
                    experiment_test_secs REAL,\
                    binary_size_before INTEGER,\
                    binary_size_after INTEGER,\
                    model_used TEXT DEFAULT 'mlx-qwen-7b',\
                    proposal TEXT,\
                    error_message TEXT,\
                    created_at TEXT DEFAULT (datetime('now')),\
                    completed_at TEXT\
                );\
                CREATE INDEX IF NOT EXISTS idx_ar_status \
                    ON autoresearch_experiments(status);\
                CREATE INDEX IF NOT EXISTS idx_ar_outcome \
                    ON autoresearch_experiments(outcome);\
                CREATE TABLE IF NOT EXISTS autoresearch_metrics (\
                    id INTEGER PRIMARY KEY,\
                    test_count INTEGER,\
                    test_duration_secs REAL,\
                    binary_size_bytes INTEGER,\
                    total_rust_lines INTEGER,\
                    crate_count INTEGER,\
                    collected_at TEXT DEFAULT (datetime('now'))\
                );",
        }]
    }

    fn health(&self) -> Health {
        match self.pool.get() {
            Ok(_) => Health::Ok,
            Err(e) => Health::Degraded {
                reason: format!("db: {e}"),
            },
        }
    }

    fn metrics(&self) -> Vec<Metric> {
        let exp_count: f64 = self
            .pool
            .get()
            .ok()
            .and_then(|c| {
                c.query_row("SELECT COUNT(*) FROM autoresearch_experiments", [], |r| {
                    r.get::<_, i64>(0)
                })
                .ok()
            })
            .unwrap_or(0) as f64;
        vec![Metric {
            name: "autoresearch_experiments_total".to_string(),
            value: exp_count,
            labels: vec![],
        }]
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![ScheduledTask {
            name: "autoresearch-nightly",
            cron: "0 2 * * *",
        }]
    }

    fn on_scheduled_task(&self, task_name: &str) {
        if task_name == "autoresearch-nightly" {
            let pool = self.pool.clone();
            let config = self.config.clone();
            tokio::spawn(async move {
                crate::runner::run_cycle(&pool, &config).await;
            });
        }
    }

    fn mcp_tools(&self) -> Vec<McpToolDef> {
        crate::mcp_defs::autoresearch_tools()
    }
}
