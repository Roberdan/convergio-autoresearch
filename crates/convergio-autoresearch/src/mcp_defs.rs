//! MCP tool definitions for the autoresearch extension.

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn autoresearch_tools() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "cvg_autoresearch_trigger".into(),
            description: "Trigger an autoresearch run.".into(),
            method: "POST".into(),
            path: "/api/autoresearch/trigger".into(),
            input_schema: json!({"type": "object", "properties": {"topic": {"type": "string", "description": "Research topic"}}, "required": ["topic"]}),
            min_ring: "trusted".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_autoresearch_results".into(),
            description: "Get autoresearch results.".into(),
            method: "GET".into(),
            path: "/api/autoresearch/results".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_autoresearch_experiments".into(),
            description: "List autoresearch experiments.".into(),
            method: "GET".into(),
            path: "/api/autoresearch/experiments".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_autoresearch_metrics".into(),
            description: "Get autoresearch metrics.".into(),
            method: "GET".into(),
            path: "/api/autoresearch/metrics".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
    ]
}
