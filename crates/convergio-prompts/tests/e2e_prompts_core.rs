//! E2E tests: prompt CRUD, versioning, template rendering, A/B testing,
//! spawn immutability, optimizer, and skill registry.

use std::collections::HashMap;

use convergio_prompts::types::{PromptInput, PromptQuery, PromptVariable, SkillInput, SkillQuery};
use convergio_prompts::{ab_test, optimizer, render, skills, spawn, store};
use rusqlite::Connection;

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

fn sample_input(name: &str, body: &str) -> PromptInput {
    PromptInput {
        name: name.into(),
        body: body.into(),
        variables: vec![],
        category: Some("test".into()),
    }
}

fn var(name: &str, required: bool, default: Option<&str>) -> PromptVariable {
    PromptVariable {
        name: name.into(),
        description: format!("{name} variable"),
        required,
        default_value: default.map(String::from),
    }
}

// ── prompt CRUD ──────────────────────────────────────────────────────────────

#[test]
fn prompt_create_version_deactivate_delete() {
    let conn = setup_db();
    let id1 = store::create_prompt(&conn, &sample_input("lifecycle", "v1")).unwrap();
    let p1 = store::get_prompt(&conn, &id1).unwrap();
    assert_eq!(p1.version, 1);
    assert!(p1.active);

    let id2 = store::create_prompt(&conn, &sample_input("lifecycle", "v2")).unwrap();
    assert!(!store::get_prompt(&conn, &id1).unwrap().active);
    assert!(store::get_prompt(&conn, &id2).unwrap().active);
    assert_eq!(store::get_prompt(&conn, &id2).unwrap().version, 2);

    let active = store::get_active_prompt(&conn, "lifecycle").unwrap();
    assert_eq!(active.id, id2);

    assert!(store::delete_prompt(&conn, &id2).unwrap());
    assert!(store::get_prompt(&conn, &id2).is_err());
}

#[test]
fn prompt_list_filters() {
    let conn = setup_db();
    store::create_prompt(
        &conn,
        &PromptInput {
            name: "alpha".into(),
            body: "a".into(),
            variables: vec![],
            category: Some("system".into()),
        },
    )
    .unwrap();
    store::create_prompt(
        &conn,
        &PromptInput {
            name: "beta".into(),
            body: "b".into(),
            variables: vec![],
            category: Some("task".into()),
        },
    )
    .unwrap();

    assert_eq!(
        store::list_prompts(&conn, &PromptQuery::default())
            .unwrap()
            .len(),
        2
    );
    let by_cat = store::list_prompts(
        &conn,
        &PromptQuery {
            category: Some("system".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_cat.len(), 1);
    assert_eq!(by_cat[0].name, "alpha");
}

// ── template rendering ───────────────────────────────────────────────────────

#[test]
fn render_with_defaults() {
    let vars = vec![
        var("role", true, None),
        var("org", false, Some("Convergio")),
    ];
    let mut values = HashMap::new();
    values.insert("role".into(), "engineer".into());
    let result = render::render("You are {{role}} at {{org}}.", &vars, &values).unwrap();
    assert_eq!(result, "You are engineer at Convergio.");
}

#[test]
fn render_fails_on_missing_required() {
    let err = render::render("Hi {{name}}", &[var("name", true, None)], &HashMap::new());
    assert!(err.unwrap_err().contains("name"));
}

#[test]
fn render_removes_optional_no_default() {
    let result = render::render("a{{x}}b", &[var("x", false, None)], &HashMap::new()).unwrap();
    assert_eq!(result, "ab");
}

#[test]
fn token_estimation() {
    let t = render::estimate_tokens("The quick brown fox jumps.");
    assert!((5..=15).contains(&t), "got {t}");
}

// ── A/B testing ──────────────────────────────────────────────────────────────

#[test]
fn ab_test_lifecycle() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO prompt_templates (id,name,version,body) VALUES ('pa','a',1,'A');
         INSERT INTO prompt_templates (id,name,version,body) VALUES ('pb','b',1,'B');",
    )
    .unwrap();

    let tid = ab_test::create_test(&conn, "pa", "pb", "task-e2e").unwrap();
    ab_test::record_variant_a(&conn, &tid, 400, 0.80).unwrap();
    ab_test::record_variant_b(&conn, &tid, 300, 0.92).unwrap();
    ab_test::declare_winner(&conn, &tid, "pb").unwrap();

    let result = ab_test::get_test(&conn, &tid).unwrap();
    assert_eq!(result.winner.as_deref(), Some("pb"));
    assert_eq!(result.tokens_a, 400);
    assert!((result.score_b - 0.92).abs() < 0.001);
}

#[test]
fn ab_test_list_by_task() {
    let conn = setup_db();
    conn.execute_batch(
        "INSERT INTO prompt_templates (id,name,version,body) VALUES ('x','x',1,'X');
         INSERT INTO prompt_templates (id,name,version,body) VALUES ('y','y',1,'Y');",
    )
    .unwrap();
    ab_test::create_test(&conn, "x", "y", "task-f").unwrap();
    ab_test::create_test(&conn, "x", "y", "task-f").unwrap();
    ab_test::create_test(&conn, "x", "y", "task-o").unwrap();
    assert_eq!(
        ab_test::list_tests_for_task(&conn, "task-f").unwrap().len(),
        2
    );
}

// ── spawn immutability ───────────────────────────────────────────────────────

#[test]
fn spawn_freezes_prompt() {
    let conn = setup_db();
    store::create_prompt(
        &conn,
        &PromptInput {
            name: "spawn-test".into(),
            body: "Hello {{name}}, role: {{role}}".into(),
            variables: vec![var("name", true, None), var("role", true, None)],
            category: Some("system".into()),
        },
    )
    .unwrap();

    let mut vals = HashMap::new();
    vals.insert("name".into(), "Elena".into());
    vals.insert("role".into(), "reviewer".into());
    let sp = spawn::inject_at_spawn(&conn, "spawn-test", "ag-1", "t-5", &vals).unwrap();
    assert!(sp.rendered_body.contains("Elena"));

    store::create_prompt(&conn, &sample_input("spawn-test", "CHANGED")).unwrap();
    let frozen = spawn::get_spawned(&conn, &sp.spawn_id).unwrap();
    assert!(frozen.rendered_body.contains("Elena"));
    assert!(!frozen.rendered_body.contains("CHANGED"));
}

// ── optimizer ────────────────────────────────────────────────────────────────

#[test]
fn optimizer_tracks_and_suggests() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO prompt_templates (id,name,version,body) VALUES ('pt-o','opt',1,'body')",
        [],
    )
    .unwrap();
    optimizer::record_usage(&conn, "pt-o", 100).unwrap();
    optimizer::record_usage(&conn, "pt-o", 200).unwrap();
    let stats = optimizer::get_stats(&conn, "pt-o").unwrap();
    assert_eq!(stats.total_tokens, 300);
    assert_eq!(stats.usage_count, 2);

    let suggestions = optimizer::suggest_optimizations(&"word ".repeat(3000));
    assert!(!suggestions.is_empty());
}

// ── skill registry ───────────────────────────────────────────────────────────

#[test]
fn skill_register_search_best_agent() {
    let conn = setup_db();
    let mk = |agent: &str, conf: f64| SkillInput {
        agent: agent.into(),
        host: "mac".into(),
        capability: "rust".into(),
        confidence: conf,
        description: "dev".into(),
    };
    skills::register_skill(&conn, &mk("coder", 0.9)).unwrap();
    skills::register_skill(&conn, &mk("reviewer", 0.7)).unwrap();

    let q = SkillQuery {
        capability: Some("rust".into()),
        ..Default::default()
    };
    assert_eq!(skills::search_skills(&conn, &q).unwrap().len(), 2);

    let best = skills::find_best_agent(&conn, "rust").unwrap().unwrap();
    assert_eq!(best.agent, "coder");
    assert!((best.confidence - 0.9).abs() < 0.01);
}
