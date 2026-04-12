//! E2E tests: pipeline CRUD and HTTP API routes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use convergio_prompts::pipeline;
use convergio_prompts::types::{PipelineInput, PipelineStep};
use rusqlite::Connection;
use tower::ServiceExt;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    convergio_db::migration::ensure_registry(&conn).unwrap();
    convergio_db::migration::apply_migrations(
        &conn,
        "prompts",
        &convergio_prompts::schema::migrations(),
    )
    .unwrap();
    conn
}

fn setup_pool() -> convergio_db::pool::ConnPool {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    {
        let conn = pool.get().unwrap();
        convergio_db::migration::ensure_registry(&conn).unwrap();
        convergio_db::migration::apply_migrations(
            &conn,
            "prompts",
            &convergio_prompts::schema::migrations(),
        )
        .unwrap();
    }
    pool
}

fn build_app(pool: convergio_db::pool::ConnPool) -> axum::Router {
    convergio_prompts::routes::routes(pool)
}

async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn post_json(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn delete(uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

// ── pipeline CRUD ────────────────────────────────────────────────────────────

#[test]
fn pipeline_create_get_list_delete() {
    let conn = setup_db();
    let input = PipelineInput {
        name: "e2e-flow".into(),
        description: "Test pipeline".into(),
        steps: vec![
            PipelineStep {
                order: 1,
                skill: "research".into(),
                prompt_name: "skill-research".into(),
                agent: None,
                condition: None,
            },
            PipelineStep {
                order: 2,
                skill: "execute".into(),
                prompt_name: "skill-execute".into(),
                agent: Some("baccio".into()),
                condition: Some("research.ok".into()),
            },
        ],
    };
    let id = pipeline::create_pipeline(&conn, &input).unwrap();
    assert!(id.starts_with("pl-"));

    let got = pipeline::get_pipeline(&conn, "e2e-flow").unwrap();
    assert_eq!(got.steps.len(), 2);
    assert_eq!(got.steps[1].agent.as_deref(), Some("baccio"));
    assert_eq!(pipeline::list_pipelines(&conn).unwrap().len(), 1);
    assert!(pipeline::delete_pipeline(&conn, "e2e-flow").unwrap());
    assert!(pipeline::list_pipelines(&conn).unwrap().is_empty());
}

// ── API routes ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn api_create_and_get_prompt() {
    let pool = setup_pool();
    let body = serde_json::json!({
        "name": "e2e-prompt", "body": "You are {{role}}.",
        "variables": [{"name":"role","description":"Role","required":true}],
        "category": "test"
    });
    let resp = build_app(pool.clone())
        .oneshot(post_json("/api/prompts", &body.to_string()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    let id = json["id"].as_str().unwrap();
    assert!(id.starts_with("pt-"));

    let resp2 = build_app(pool.clone())
        .oneshot(get(&format!("/api/prompts/{id}")))
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let p = body_json(resp2).await;
    assert_eq!(p["name"], "e2e-prompt");
    assert_eq!(p["version"], 1);
}

#[tokio::test]
async fn api_list_and_delete_prompt() {
    let pool = setup_pool();
    let body = serde_json::json!({
        "name": "listed", "body": "body", "variables": []
    });
    let resp = build_app(pool.clone())
        .oneshot(post_json("/api/prompts", &body.to_string()))
        .await
        .unwrap();
    let id = body_json(resp).await["id"].as_str().unwrap().to_string();

    let resp2 = build_app(pool.clone())
        .oneshot(get("/api/prompts"))
        .await
        .unwrap();
    assert!(!body_json(resp2).await.as_array().unwrap().is_empty());

    let resp3 = build_app(pool.clone())
        .oneshot(delete(&format!("/api/prompts/{id}")))
        .await
        .unwrap();
    assert_eq!(resp3.status(), StatusCode::NO_CONTENT);

    let resp4 = build_app(pool.clone())
        .oneshot(get(&format!("/api/prompts/{id}")))
        .await
        .unwrap();
    assert_eq!(resp4.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn api_register_and_search_skill() {
    let pool = setup_pool();
    let body = serde_json::json!({
        "agent": "e2e-agent", "host": "test-host",
        "capability": "testing", "confidence": 0.85,
        "description": "E2E skill"
    });
    let resp = build_app(pool.clone())
        .oneshot(post_json("/api/skills", &body.to_string()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(body_json(resp).await["id"]
        .as_str()
        .unwrap()
        .starts_with("sk-"));

    let resp2 = build_app(pool.clone())
        .oneshot(get("/api/skills?capability=testing"))
        .await
        .unwrap();
    let arr = body_json(resp2).await;
    assert_eq!(arr.as_array().unwrap().len(), 1);
    assert_eq!(arr[0]["agent"], "e2e-agent");
}

#[tokio::test]
async fn api_active_prompt_by_name() {
    let pool = setup_pool();
    let body = serde_json::json!({
        "name": "named-p", "body": "v1", "variables": []
    });
    build_app(pool.clone())
        .oneshot(post_json("/api/prompts", &body.to_string()))
        .await
        .unwrap();

    let resp = build_app(pool.clone())
        .oneshot(get("/api/prompts/active/named-p"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["name"], "named-p");
    assert_eq!(json["active"], true);
}
