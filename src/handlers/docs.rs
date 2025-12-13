// handlers/docs.rs
// Documentation and health handlers: openapi, docs, health, features

use crate::models::{HealthResponse, LastAction, ZfsFeaturesResponse};
use serde_json::{json, Map, Value};
use std::sync::{Arc, RwLock};
use warp::{Rejection, Reply};

// Embed templates at compile time
const API_SPEC: &str = include_str!("../../api.json");
const FEATURES_TEMPLATE: &str = include_str!("../../templates/features.html");
const DOCS_TEMPLATE: &str = include_str!("../../templates/docs.html");

// URL-encoded SVG favicon (ZFSreload light logo)
const FAVICON_SVG_ENCODED: &str = "%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20512%20512%22%3E%3Cdefs%3E%3ClinearGradient%20id%3D%22g%22%20x1%3D%220%25%22%20y1%3D%220%25%22%20x2%3D%22100%25%22%20y2%3D%22100%25%22%3E%3Cstop%20offset%3D%220%25%22%20stop-color%3D%22%23FDFCFB%22%2F%3E%3Cstop%20offset%3D%22100%25%22%20stop-color%3D%22%23E2E0DD%22%2F%3E%3C%2FlinearGradient%3E%3C%2Fdefs%3E%3Crect%20x%3D%2232%22%20y%3D%2232%22%20width%3D%22448%22%20height%3D%22448%22%20rx%3D%22100%22%20fill%3D%22url(%23g)%22%2F%3E%3Cpath%20d%3D%22M170%20170.5H340L170%20317.5H340%22%20stroke%3D%22%233D0E1A%22%20stroke-width%3D%2238%22%20fill%3D%22none%22%2F%3E%3Ctext%20x%3D%22344%22%20y%3D%22336%22%20font-family%3D%22sans-serif%22%20font-size%3D%22200%22%20font-weight%3D%22bold%22%20fill%3D%22%236B1A30%22%3Er%3C%2Ftext%3E%3C%2Fsvg%3E";

/// Generate OpenAPI 3.0 spec from our lean api.json
fn generate_openapi() -> Value {
    let api: Value = serde_json::from_str(API_SPEC).expect("Invalid api.json");

    let version = api["version"].as_str().unwrap_or("0.0.0");
    let title = api["title"].as_str().unwrap_or("API");
    let base_url = api["base_url"].as_str().unwrap_or("/v1");
    let auth_header = api["auth"]["header"].as_str().unwrap_or("X-API-Key");
    let auth_desc = api["auth"]["description"].as_str().unwrap_or("");

    let mut paths: Map<String, Value> = Map::new();
    let mut tags_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(endpoints) = api["endpoints"].as_array() {
        for endpoint in endpoints {
            let method = endpoint["method"].as_str().unwrap_or("GET").to_lowercase();
            let path = endpoint["path"].as_str().unwrap_or("/");
            let tag = endpoint["tag"].as_str().unwrap_or("default");
            let summary = endpoint["summary"].as_str().unwrap_or("");
            let description = endpoint["description"].as_str().unwrap_or("");
            let auth = endpoint["auth"].as_bool().unwrap_or(true);

            tags_set.insert(tag.to_string());

            // Build parameters array
            let mut parameters: Vec<Value> = vec![];
            if let Some(params) = endpoint["params"].as_object() {
                for (name, param) in params {
                    let location = param["in"].as_str().unwrap_or("query");
                    let param_type = param["type"].as_str().unwrap_or("string");
                    let required = param["required"].as_bool().unwrap_or(location == "path");
                    let param_desc = param["description"].as_str().unwrap_or("");

                    let mut param_obj = json!({
                        "name": name,
                        "in": location,
                        "required": required,
                        "schema": {"type": param_type}
                    });

                    if !param_desc.is_empty() {
                        param_obj["description"] = json!(param_desc);
                    }
                    if let Some(default) = param.get("default") {
                        param_obj["schema"]["default"] = default.clone();
                    }
                    if let Some(enum_vals) = param.get("enum") {
                        param_obj["schema"]["enum"] = enum_vals.clone();
                    }

                    parameters.push(param_obj);
                }
            }

            // Build request body if present
            let request_body = if let Some(body) = endpoint["body"].as_object() {
                let mut properties: Map<String, Value> = Map::new();
                let mut required_fields: Vec<Value> = vec![];

                for (name, field) in body {
                    let field_type = field["type"].as_str().unwrap_or("string");
                    let mut prop = json!({"type": field_type});

                    if let Some(desc) = field.get("description") {
                        prop["description"] = desc.clone();
                    }
                    if let Some(example) = field.get("example") {
                        prop["example"] = example.clone();
                    }
                    if let Some(enum_vals) = field.get("enum") {
                        prop["enum"] = enum_vals.clone();
                    }
                    if let Some(default) = field.get("default") {
                        prop["default"] = default.clone();
                    }
                    if field_type == "array" {
                        if let Some(items) = field.get("items") {
                            prop["items"] = json!({"type": items});
                        }
                    }
                    if field_type == "object" {
                        if field.get("properties").is_some() {
                            prop["additionalProperties"] = json!(true);
                        }
                    }

                    if field["required"].as_bool().unwrap_or(false) {
                        required_fields.push(json!(name));
                    }

                    properties.insert(name.clone(), prop);
                }

                let mut schema = json!({
                    "type": "object",
                    "properties": properties
                });
                if !required_fields.is_empty() {
                    schema["required"] = json!(required_fields);
                }

                Some(json!({
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": schema
                        }
                    }
                }))
            } else {
                None
            };

            // Build response schema
            let response_schema = if let Some(resp) = endpoint["response"].as_object() {
                let mut properties: Map<String, Value> = Map::new();
                for (name, field) in resp {
                    if name == "_note" {
                        continue;
                    }
                    let prop = convert_field_to_openapi(field);
                    properties.insert(name.clone(), prop);
                }
                json!({
                    "type": "object",
                    "properties": properties
                })
            } else {
                json!({"type": "object"})
            };

            // Build operation object
            let mut operation = json!({
                "summary": summary,
                "description": description,
                "tags": [tag],
                "responses": {
                    "200": {
                        "description": "Success",
                        "content": {
                            "application/json": {
                                "schema": response_schema
                            }
                        }
                    }
                }
            });

            if !parameters.is_empty() {
                operation["parameters"] = json!(parameters);
            }
            if let Some(body) = request_body {
                operation["requestBody"] = body;
            }
            if !auth {
                operation["security"] = json!([]);
            }

            // Add to paths
            let path_entry = paths.entry(path.to_string()).or_insert(json!({}));
            path_entry[method] = operation;
        }
    }

    // Build tags array
    let tags: Vec<Value> = tags_set
        .into_iter()
        .map(|t| {
            json!({
                "name": t,
                "description": format!("{} operations", t.to_uppercase())
            })
        })
        .collect();

    json!({
        "openapi": "3.0.3",
        "info": {
            "title": title,
            "version": version,
            "description": format!("RESTful API for managing ZFS pools, datasets, and snapshots.\n\n**AI/Programmatic access:** `GET /v1/docs?format=json`"),
            "license": {"name": "MIT"}
        },
        "servers": [{"url": format!("http://localhost:9876{}", base_url), "description": "Local server"}],
        "security": [{"ApiKeyAuth": []}],
        "components": {
            "securitySchemes": {
                "ApiKeyAuth": {
                    "type": "apiKey",
                    "in": "header",
                    "name": auth_header,
                    "description": auth_desc
                }
            }
        },
        "tags": tags,
        "paths": paths
    })
}

/// Convert a field from api.json format to OpenAPI schema format
fn convert_field_to_openapi(field: &Value) -> Value {
    if let Some(obj) = field.as_object() {
        let field_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("string");
        let mut prop = json!({"type": field_type});

        if let Some(desc) = obj.get("description") {
            prop["description"] = desc.clone();
        }
        if let Some(example) = obj.get("example") {
            prop["example"] = example.clone();
        }
        if let Some(enum_vals) = obj.get("enum") {
            prop["enum"] = enum_vals.clone();
        }
        if let Some(nullable) = obj.get("nullable") {
            prop["nullable"] = nullable.clone();
        }
        if field_type == "array" {
            if let Some(items) = obj.get("items") {
                if items.is_string() {
                    prop["items"] = json!({"type": items});
                } else {
                    prop["items"] = convert_field_to_openapi(items);
                }
            }
        }
        if field_type == "object" {
            if let Some(properties) = obj.get("properties") {
                if let Some(props_obj) = properties.as_object() {
                    let mut converted: Map<String, Value> = Map::new();
                    for (k, v) in props_obj {
                        converted.insert(k.clone(), convert_field_to_openapi(v));
                    }
                    prop["properties"] = json!(converted);
                }
            }
        }
        prop
    } else {
        json!({"type": "string"})
    }
}

/// Serve OpenAPI spec (generated from api.json)
pub async fn openapi_handler() -> Result<impl Reply, Rejection> {
    let openapi = generate_openapi();
    let json_str = serde_json::to_string_pretty(&openapi).unwrap_or_default();

    Ok(warp::reply::with_header(
        json_str,
        "Content-Type",
        "application/json",
    ))
}

/// Serve Swagger UI HTML page or lean API JSON
/// Returns HTML by default, api.json if ?format=json
pub async fn docs_handler(format: Option<String>) -> Result<Box<dyn Reply>, Rejection> {
    // Return lean api.json if explicitly requested
    if format.as_deref() == Some("json") {
        return Ok(Box::new(warp::reply::with_header(
            API_SPEC,
            "Content-Type",
            "application/json",
        )));
    }

    // Return Swagger UI HTML
    let html = DOCS_TEMPLATE.replace("{{FAVICON}}", FAVICON_SVG_ENCODED);
    Ok(Box::new(warp::reply::with_header(
        html,
        "Content-Type",
        "text/html; charset=utf-8",
    )))
}

pub async fn health_check_handler(
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    let last_action_data = last_action.read().unwrap().clone();

    let response = HealthResponse {
        status: "success".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        last_action: last_action_data,
    };

    Ok(warp::reply::json(&response))
}

/// Return all ZFS features with implementation status
/// No authentication required - informational endpoint
/// Returns HTML by default, JSON if ?format=json
pub async fn zfs_features_handler(format: Option<String>) -> Result<Box<dyn Reply>, Rejection> {
    let response = ZfsFeaturesResponse::build();

    // Return JSON if explicitly requested
    if format.as_deref() == Some("json") {
        return Ok(Box::new(warp::reply::json(&response)));
    }

    // Build HTML view
    let html = build_features_html(&response);
    Ok(Box::new(warp::reply::html(html)))
}

/// Build a visually appealing HTML page for ZFS features
fn build_features_html(data: &ZfsFeaturesResponse) -> String {
    let mut features_html = String::new();

    // Group features by category
    let categories = [
        ("pool", "Pool Operations", "&#128692;"),
        ("dataset", "Dataset Operations", "&#128193;"),
        ("snapshot", "Snapshot Operations", "&#128248;"),
        ("property", "Properties", "&#9881;"),
        ("replication", "Replication", "&#128260;"),
        ("system", "System", "&#128421;"),
    ];

    for (cat_key, cat_name, cat_icon) in categories {
        let cat_features: Vec<_> = data
            .features
            .iter()
            .filter(|f| format!("{:?}", f.category).to_lowercase() == cat_key)
            .collect();

        if cat_features.is_empty() {
            continue;
        }

        let implemented_count = cat_features.iter().filter(|f| f.implemented).count();

        features_html.push_str(&format!(
            r#"
        <div class="category">
            <div class="category-header">
                <span class="category-icon">{}</span>
                <span class="category-name">{}</span>
                <span class="category-count">{}/{}</span>
            </div>
            <div class="features-grid">
        "#,
            cat_icon,
            cat_name,
            implemented_count,
            cat_features.len()
        ));

        for feature in cat_features {
            let status_class = if feature.implemented {
                "implemented"
            } else {
                "planned"
            };
            let status_icon = if feature.implemented {
                "&#10003;"
            } else {
                "&#9675;"
            };

            let impl_badge = match &feature.implementation {
                Some(m) => {
                    let (badge_class, badge_text) = match format!("{:?}", m).to_lowercase().as_str()
                    {
                        "libzetta" => ("libzetta", "libzetta"),
                        "ffi" => ("ffi", "FFI"),
                        "libzfs" => ("libzfs", "libzfs"),
                        "cliexperimental" => ("cli", "CLI"),
                        "hybrid" => ("hybrid", "Hybrid"),
                        _ => ("planned", "Planned"),
                    };
                    format!(
                        r#"<span class="impl-badge {}">{}</span>"#,
                        badge_class, badge_text
                    )
                }
                None => String::new(),
            };

            let endpoint_html = feature
                .endpoint
                .as_ref()
                .map(|e| format!(r#"<div class="endpoint"><code>{}</code></div>"#, e))
                .unwrap_or_default();

            let notes_html = feature
                .notes
                .as_ref()
                .map(|n| format!(r#"<div class="notes">{}</div>"#, n))
                .unwrap_or_default();

            features_html.push_str(&format!(
                r#"
                <div class="feature-card {}">
                    <div class="feature-header">
                        <span class="status-icon">{}</span>
                        <span class="feature-name">{}</span>
                        {}
                    </div>
                    {}
                    {}
                </div>
            "#,
                status_class, status_icon, feature.name, impl_badge, endpoint_html, notes_html
            ));
        }

        features_html.push_str("</div></div>");
    }

    // Replace all template placeholders
    FEATURES_TEMPLATE
        .replace("{{FAVICON}}", FAVICON_SVG_ENCODED)
        .replace("{{TOTAL}}", &data.summary.total.to_string())
        .replace("{{IMPLEMENTED}}", &data.summary.implemented.to_string())
        .replace("{{PLANNED}}", &data.summary.planned.to_string())
        .replace("{{FEATURES}}", &features_html)
        .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
}
