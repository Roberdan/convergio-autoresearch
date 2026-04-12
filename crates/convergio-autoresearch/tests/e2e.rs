//! E2E tests for convergio-autoresearch route handlers.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use convergio_autoresearch::routes::{autoresearch_routes, AutoresearchState};
use convergio_autoresearch::types::AutoresearchConfig;
use convergio_db::pool::ConnPool;
use convergio_types::extension::Extension;
use tower::ServiceExt;

fn setup() -> (axum::Router, ConnPool) {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    let ext = convergio_autoresearch::AutoresearchExtension::new(pool.clone());
    for m in ext.migrations() {
        conn.execute_batch(m.up).unwrap();
    }
    drop(conn);
    let state = Arc::new(AutoresearchState {
        pool: pool.clone(),
        config: AutoresearchConfig::default(),
    });
    (autoresearch_routes(state), pool)
}

fn rebuild(pool: &ConnPool) -> axum::Router {
    let state = Arc::new(AutoresearchState {
        pool: pool.clone(),
        config: AutoresearchConfig::default(),
    });
    autoresearch_routes(state)
}

async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn post_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn seed_experiment(pool: &ConnPool, outcome: &str) {
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO autoresearch_experiments \
         (target_file, description, status, outcome, model_used, proposal) \
         VALUES ('lib.rs', 'optimize alloc', 'completed', ?1, 'test-model', 'use Vec')",
        rusqlite::params![outcome],
    )
    .unwrap();
}

fn seed_metric(pool: &ConnPool) {
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO autoresearch_metrics \
         (test_count, test_duration_secs, binary_size_bytes, total_rust_lines, crate_count) \
         VALUES (100, 5.5, 2000000, 50000, 28)",
        [],
    )
    .unwrap();
}

// --- Results route tests ---

#[tokio::test]
async fn results_empty_db() {
    let (app, _) = setup();
    let resp = app
        .oneshot(get_req("/api/autoresearch/results"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total_experiments"], 0);
    assert_eq!(json["kept"], 0);
    assert_eq!(json["discarded"], 0);
    assert_eq!(json["errors"], 0);
    assert_eq!(json["success_rate"], 0.0);
    assert!(json["model"].as_str().unwrap().contains("Qwen"));
}

#[tokio::test]
async fn results_with_experiments() {
    let (_, pool) = setup();
    seed_experiment(&pool, "kept");
    seed_experiment(&pool, "kept");
    seed_experiment(&pool, "discarded");
    seed_experiment(&pool, "error");

    let app = rebuild(&pool);
    let resp = app
        .oneshot(get_req("/api/autoresearch/results"))
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["total_experiments"], 4);
    assert_eq!(json["kept"], 2);
    assert_eq!(json["discarded"], 1);
    assert_eq!(json["errors"], 1);
    assert_eq!(json["success_rate"], 0.5);
}

// --- Experiments listing tests ---

#[tokio::test]
async fn experiments_empty_db() {
    let (app, _) = setup();
    let resp = app
        .oneshot(get_req("/api/autoresearch/experiments"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["experiments"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn experiments_with_data() {
    let (_, pool) = setup();
    seed_experiment(&pool, "kept");
    seed_experiment(&pool, "discarded");

    let app = rebuild(&pool);
    let resp = app
        .oneshot(get_req("/api/autoresearch/experiments"))
        .await
        .unwrap();
    let json = body_json(resp).await;
    let exps = json["experiments"].as_array().unwrap();
    assert_eq!(exps.len(), 2);
    assert_eq!(exps[0]["target_file"], "lib.rs");
    assert_eq!(exps[0]["status"], "completed");
}

#[tokio::test]
async fn experiments_pagination() {
    let (_, pool) = setup();
    for _ in 0..5 {
        seed_experiment(&pool, "kept");
    }

    let app = rebuild(&pool);
    let resp = app
        .oneshot(get_req("/api/autoresearch/experiments?limit=2&offset=0"))
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["experiments"].as_array().unwrap().len(), 2);

    let app2 = rebuild(&pool);
    let resp2 = app2
        .oneshot(get_req("/api/autoresearch/experiments?limit=2&offset=3"))
        .await
        .unwrap();
    let json2 = body_json(resp2).await;
    assert_eq!(json2["experiments"].as_array().unwrap().len(), 2);
}

// --- Metrics route tests ---

#[tokio::test]
async fn metrics_empty_db() {
    let (app, _) = setup();
    let resp = app
        .oneshot(get_req("/api/autoresearch/metrics"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["metrics"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn metrics_with_data() {
    let (_, pool) = setup();
    seed_metric(&pool);
    seed_metric(&pool);

    let app = rebuild(&pool);
    let resp = app
        .oneshot(get_req("/api/autoresearch/metrics"))
        .await
        .unwrap();
    let json = body_json(resp).await;
    let rows = json["metrics"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["test_count"], 100);
    assert_eq!(rows[0]["crate_count"], 28);
}

// --- Trigger route test ---

#[tokio::test]
async fn trigger_returns_ok() {
    let (app, _) = setup();
    let resp = app
        .oneshot(post_req("/api/autoresearch/trigger"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["ok"], true);
    assert!(json["message"].as_str().unwrap().contains("triggered"));
}
