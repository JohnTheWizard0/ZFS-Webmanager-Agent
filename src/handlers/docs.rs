// handlers/docs.rs
// Documentation and health handlers: openapi, docs, health, features

use crate::models::{HealthResponse, LastAction, ZfsFeaturesResponse};
use std::sync::{Arc, RwLock};
use warp::{Rejection, Reply};

// Embed templates at compile time
const OPENAPI_SPEC: &str = include_str!("../../openapi.yaml");
const FEATURES_TEMPLATE: &str = include_str!("../../templates/features.html");
const DOCS_TEMPLATE: &str = include_str!("../../templates/docs.html");

// URL-encoded SVG favicon (ZFSreload light logo)
const FAVICON_SVG_ENCODED: &str = "%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20512%20512%22%3E%3Cdefs%3E%3ClinearGradient%20id%3D%22g%22%20x1%3D%220%25%22%20y1%3D%220%25%22%20x2%3D%22100%25%22%20y2%3D%22100%25%22%3E%3Cstop%20offset%3D%220%25%22%20stop-color%3D%22%23FDFCFB%22%2F%3E%3Cstop%20offset%3D%22100%25%22%20stop-color%3D%22%23E2E0DD%22%2F%3E%3C%2FlinearGradient%3E%3C%2Fdefs%3E%3Crect%20x%3D%2232%22%20y%3D%2232%22%20width%3D%22448%22%20height%3D%22448%22%20rx%3D%22100%22%20fill%3D%22url(%23g)%22%2F%3E%3Cpath%20d%3D%22M170%20170.5H340L170%20317.5H340%22%20stroke%3D%22%233D0E1A%22%20stroke-width%3D%2238%22%20fill%3D%22none%22%2F%3E%3Ctext%20x%3D%22344%22%20y%3D%22336%22%20font-family%3D%22sans-serif%22%20font-size%3D%22200%22%20font-weight%3D%22bold%22%20fill%3D%22%236B1A30%22%3Er%3C%2Ftext%3E%3C%2Fsvg%3E";

/// Serve OpenAPI spec as YAML
pub async fn openapi_handler() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_header(
        OPENAPI_SPEC,
        "Content-Type",
        "application/yaml",
    ))
}

/// Serve Swagger UI HTML page
pub async fn docs_handler() -> Result<impl Reply, Rejection> {
    let html = DOCS_TEMPLATE.replace("{{FAVICON}}", FAVICON_SVG_ENCODED);

    Ok(warp::reply::with_header(
        html,
        "Content-Type",
        "text/html; charset=utf-8",
    ))
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
