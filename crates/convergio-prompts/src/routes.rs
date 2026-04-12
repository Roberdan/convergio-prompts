//! HTTP routes for prompt management and skill registry.
//!
//! Mounts under `/api/prompts` and `/api/skills`.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use convergio_db::pool::ConnPool;

use crate::types::{PromptInput, PromptQuery, SkillInput, SkillQuery};

/// Structured error response — never leaks internal details.
fn internal_err() -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal error".to_string(),
    )
}

fn validation_err(msg: &str) -> (StatusCode, String) {
    (StatusCode::UNPROCESSABLE_ENTITY, msg.to_string())
}

/// Build all prompt + skill routes.
pub fn routes(pool: ConnPool) -> Router {
    Router::new()
        .route("/api/prompts", post(create_prompt).get(list_prompts))
        .route("/api/prompts/:id", get(get_prompt).delete(delete_prompt))
        .route("/api/prompts/active/:name", get(get_active_prompt))
        .route("/api/skills", post(register_skill).get(search_skills))
        .route("/api/skills/search", get(search_skills_query))
        .with_state(pool)
}

async fn create_prompt(
    State(pool): State<ConnPool>,
    Json(input): Json<PromptInput>,
) -> impl IntoResponse {
    if let Err(e) = input.validate() {
        return Err(validation_err(&e.to_string()));
    }
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let id = crate::store::create_prompt(&conn, &input).map_err(|e| {
        tracing::error!("create_prompt failed: {e}");
        internal_err()
    })?;
    Ok::<_, (StatusCode, String)>((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn get_prompt(State(pool): State<ConnPool>, Path(id): Path<String>) -> impl IntoResponse {
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let prompt = crate::store::get_prompt(&conn, &id)
        .map_err(|_| (StatusCode::NOT_FOUND, "prompt not found".to_string()))?;
    Ok::<_, (StatusCode, String)>(Json(prompt))
}

async fn get_active_prompt(
    State(pool): State<ConnPool>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let prompt = crate::store::get_active_prompt(&conn, &name)
        .map_err(|_| (StatusCode::NOT_FOUND, "no active prompt".to_string()))?;
    Ok::<_, (StatusCode, String)>(Json(prompt))
}

async fn list_prompts(
    State(pool): State<ConnPool>,
    Query(query): Query<PromptQuery>,
) -> impl IntoResponse {
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let prompts = crate::store::list_prompts(&conn, &query).map_err(|e| {
        tracing::error!("list_prompts failed: {e}");
        internal_err()
    })?;
    Ok::<_, (StatusCode, String)>(Json(prompts))
}

async fn delete_prompt(State(pool): State<ConnPool>, Path(id): Path<String>) -> impl IntoResponse {
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let deleted = crate::store::delete_prompt(&conn, &id).map_err(|e| {
        tracing::error!("delete_prompt failed: {e}");
        internal_err()
    })?;
    if deleted {
        Ok::<_, (StatusCode, String)>(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "prompt not found".to_string()))
    }
}

async fn register_skill(
    State(pool): State<ConnPool>,
    payload: Result<Json<SkillInput>, JsonRejection>,
) -> impl IntoResponse {
    let Json(input) = payload.map_err(|e| {
        let body = serde_json::json!({
            "error": {
                "code": "VALIDATION_ERROR",
                "message": e.body_text(),
            }
        });
        (StatusCode::UNPROCESSABLE_ENTITY, Json(body).into_response())
    })?;
    if let Err(e) = input.validate() {
        let body =
            serde_json::json!({"error": {"code": "VALIDATION_ERROR", "message": e.to_string()}});
        return Err((StatusCode::UNPROCESSABLE_ENTITY, Json(body).into_response()));
    }
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        let body =
            serde_json::json!({"error": {"code": "INTERNAL_ERROR", "message": "internal error"}});
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(body).into_response(),
        )
    })?;
    let id = crate::skills::register_skill(&conn, &input).map_err(|e| {
        tracing::error!("register_skill failed: {e}");
        let body =
            serde_json::json!({"error": {"code": "INTERNAL_ERROR", "message": "internal error"}});
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(body).into_response(),
        )
    })?;
    Ok::<_, (StatusCode, axum::response::Response)>((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })).into_response(),
    ))
}

async fn search_skills(
    State(pool): State<ConnPool>,
    Query(query): Query<SkillQuery>,
) -> impl IntoResponse {
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let skills = crate::skills::search_skills(&conn, &query).map_err(|e| {
        tracing::error!("search_skills failed: {e}");
        internal_err()
    })?;
    // Also include seeded skill prompts from prompt_templates (fixes #521)
    let seeded: Vec<serde_json::Value> = conn
        .prepare(
            "SELECT name, body, category FROM prompt_templates \
             WHERE category = 'skill' AND active = 1",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(serde_json::json!({
                    "name": r.get::<_, String>(0)?,
                    "type": "seeded",
                    "category": r.get::<_, String>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let mut result: Vec<serde_json::Value> = seeded;
    for s in &skills {
        result.push(serde_json::to_value(s).unwrap_or_default());
    }
    Ok::<_, (StatusCode, String)>(Json(result))
}

async fn search_skills_query(
    State(pool): State<ConnPool>,
    Query(query): Query<SkillQuery>,
) -> impl IntoResponse {
    let conn = pool.get().map_err(|e| {
        tracing::error!("pool error: {e}");
        internal_err()
    })?;
    let skills = crate::skills::search_skills(&conn, &query).map_err(|e| {
        tracing::error!("search_skills failed: {e}");
        internal_err()
    })?;
    Ok::<_, (StatusCode, String)>(Json(skills))
}
