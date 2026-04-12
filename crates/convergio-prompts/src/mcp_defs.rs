//! MCP tool definitions for the prompts extension.

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn prompts_tools() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "cvg_list_skills".into(),
            description: "Search available skills (solve, planner, execute, etc).".into(),
            method: "GET".into(),
            path: "/api/skills".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "capability": {"type": "string", "description": "Filter by capability name"},
                    "agent": {"type": "string", "description": "Filter by agent name"}
                }
            }),
            min_ring: "sandboxed".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_get_skill_prompt".into(),
            description: "Get the active prompt template for a skill by name.".into(),
            method: "GET".into(),
            path: "/api/prompts/active/:name".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"name": {"type": "string", "description": "Skill/prompt name"}},
                "required": ["name"]
            }),
            min_ring: "community".into(),
            path_params: vec!["name".into()],
        },
    ]
}
