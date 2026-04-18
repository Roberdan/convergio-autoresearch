//! HTTP routes for autoresearch dashboard.
//!
//! - GET  /api/autoresearch/results      — experiment results + stats
//! - GET  /api/autoresearch/experiments   — list experiments (paginated)
//! - GET  /api/autoresearch/metrics       — project metrics history
//! - POST /api/autoresearch/trigger       — manually trigger a cycle

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use convergio_db::pool::ConnPool;
use rusqlite;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::runner::run_cycle;
use crate::types::AutoresearchConfig;

pub struct AutoresearchState {
    pub pool: ConnPool,
    pub config: AutoresearchConfig,
}

pub fn autoresearch_routes(state: Arc<AutoresearchState>) -> Router {
    Router::new()
        .route("/api/autoresearch/results", get(handle_results))
        .route("/api/autoresearch/experiments", get(handle_experiments))
        .route("/api/autoresearch/metrics", get(handle_metrics))
        .route("/api/autoresearch/trigger", post(handle_trigger))
        .with_state(state)
}

async fn handle_results(State(s): State<Arc<AutoresearchState>>) -> Json<Value> {
    let conn = match s.pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "pool error in results handler");
            return Json(json!({"error": "internal error"}));
        }
    };
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM autoresearch_experiments", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let kept: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM autoresearch_experiments WHERE outcome = 'kept'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let discarded: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM autoresearch_experiments WHERE outcome = 'discarded'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM autoresearch_experiments WHERE outcome = 'error'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Json(json!({
        "total_experiments": total,
        "kept": kept,
        "discarded": discarded,
        "errors": errors,
        "success_rate": if total > 0 { kept as f64 / total as f64 } else { 0.0 },
        "model": s.config.model,
    }))
}

#[derive(Deserialize, Default)]
struct ListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn handle_experiments(
    State(s): State<Arc<AutoresearchState>>,
    Query(q): Query<ListQuery>,
) -> Json<Value> {
    let conn = match s.pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "pool error in experiments handler");
            return Json(json!({"error": "internal error"}));
        }
    };
    let limit = q.limit.unwrap_or(20).min(100);
    let offset = q.offset.unwrap_or(0).min(10_000);
    let sql = "SELECT id, target_file, description, status, outcome, \
         baseline_test_secs, experiment_test_secs, model_used, \
         created_at, completed_at \
         FROM autoresearch_experiments ORDER BY id DESC LIMIT ?1 OFFSET ?2";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to prepare experiments query");
            return Json(json!({"error": "internal error"}));
        }
    };
    let rows: Vec<Value> = stmt
        .query_map(rusqlite::params![limit, offset], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "target_file": r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "status": r.get::<_, String>(3)?,
                "outcome": r.get::<_, String>(4)?,
                "baseline_test_secs": r.get::<_, Option<f64>>(5)?,
                "experiment_test_secs": r.get::<_, Option<f64>>(6)?,
                "model_used": r.get::<_, String>(7)?,
                "created_at": r.get::<_, String>(8)?,
                "completed_at": r.get::<_, Option<String>>(9)?,
            }))
        })
        .map(|rows| {
            rows.filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read experiment row");
                    None
                }
            })
            .collect()
        })
        .unwrap_or_default();
    Json(json!({"experiments": rows}))
}

async fn handle_metrics(State(s): State<Arc<AutoresearchState>>) -> Json<Value> {
    let conn = match s.pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "pool error in metrics handler");
            return Json(json!({"error": "internal error"}));
        }
    };
    let mut stmt = match conn.prepare(
        "SELECT test_count, test_duration_secs, binary_size_bytes, \
         total_rust_lines, crate_count, collected_at \
         FROM autoresearch_metrics ORDER BY id DESC LIMIT 30",
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to prepare metrics query");
            return Json(json!({"error": "internal error"}));
        }
    };
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({
                "test_count": r.get::<_, u32>(0)?,
                "test_duration_secs": r.get::<_, f64>(1)?,
                "binary_size_bytes": r.get::<_, u64>(2)?,
                "total_rust_lines": r.get::<_, u64>(3)?,
                "crate_count": r.get::<_, u32>(4)?,
                "collected_at": r.get::<_, String>(5)?,
            }))
        })
        .map(|rows| {
            rows.filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read metrics row");
                    None
                }
            })
            .collect()
        })
        .unwrap_or_default();
    Json(json!({"metrics": rows}))
}

async fn handle_trigger(State(s): State<Arc<AutoresearchState>>) -> Json<Value> {
    let pool = s.pool.clone();
    let config = s.config.clone();
    tokio::spawn(async move {
        run_cycle(&pool, &config).await;
    });
    Json(json!({"ok": true, "message": "autoresearch cycle triggered"}))
}
