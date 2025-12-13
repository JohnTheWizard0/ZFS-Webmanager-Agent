use crate::models::*;
use crate::safety::SafetyManager;
use crate::task_manager::TaskManager;
use crate::utils::{
    error_response, success_response, validate_dataset_name, validate_snapshot_name,
};
use crate::zfs_management::{RollbackError, ZfsManager};
use std::process::Command;
use std::sync::{Arc, RwLock};
use warp::{Rejection, Reply};

// Embed OpenAPI spec at compile time
const OPENAPI_SPEC: &str = include_str!("../openapi.yaml");

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
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>ZFS Web Manager API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        html { box-sizing: border-box; overflow: -moz-scrollbars-vertical; overflow-y: scroll; }
        *, *:before, *:after { box-sizing: inherit; }
        body { margin: 0; background: #fafafa; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js" charset="UTF-8"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: "/v1/openapi.yaml",
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis
                ]
            });
        };
    </script>
</body>
</html>"#;

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
        ("pool", "Pool Operations", "🏊"),
        ("dataset", "Dataset Operations", "📁"),
        ("snapshot", "Snapshot Operations", "📸"),
        ("property", "Properties", "⚙️"),
        ("replication", "Replication", "🔄"),
        ("system", "System", "🖥️"),
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
            let status_icon = if feature.implemented { "✓" } else { "○" };

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

    format!(
        r##"<!DOCTYPE html>
<html>
<head>
    <title>ZFS Agent - Feature Coverage</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        :root {{
            --bg-primary: #0f172a;
            --bg-secondary: #1e293b;
            --bg-card: #334155;
            --text-primary: #f1f5f9;
            --text-secondary: #94a3b8;
            --accent-green: #22c55e;
            --accent-blue: #3b82f6;
            --accent-purple: #a855f7;
            --accent-orange: #f97316;
            --accent-yellow: #eab308;
            --accent-gray: #64748b;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
            min-height: 100vh;
        }}
        .container {{
            max-width: 1400px;
            margin: 0 auto;
            padding: 2rem;
        }}
        header {{
            text-align: center;
            margin-bottom: 3rem;
            padding: 2rem;
            background: linear-gradient(135deg, var(--bg-secondary) 0%, var(--bg-primary) 100%);
            border-radius: 16px;
            border: 1px solid var(--bg-card);
        }}
        h1 {{
            font-size: 2.5rem;
            font-weight: 700;
            margin-bottom: 0.5rem;
            background: linear-gradient(90deg, var(--accent-blue), var(--accent-purple));
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }}
        .subtitle {{
            color: var(--text-secondary);
            font-size: 1.1rem;
        }}
        .summary {{
            display: flex;
            justify-content: center;
            gap: 2rem;
            margin-top: 1.5rem;
            flex-wrap: wrap;
        }}
        .stat {{
            background: var(--bg-card);
            padding: 1rem 2rem;
            border-radius: 12px;
            text-align: center;
        }}
        .stat-value {{
            font-size: 2rem;
            font-weight: 700;
            color: var(--accent-blue);
        }}
        .stat-value.green {{ color: var(--accent-green); }}
        .stat-value.orange {{ color: var(--accent-orange); }}
        .stat-label {{
            color: var(--text-secondary);
            font-size: 0.875rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }}
        .category {{
            margin-bottom: 2rem;
        }}
        .category-header {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
            margin-bottom: 1rem;
            padding: 0.75rem 1rem;
            background: var(--bg-secondary);
            border-radius: 8px;
        }}
        .category-icon {{
            font-size: 1.5rem;
        }}
        .category-name {{
            font-size: 1.25rem;
            font-weight: 600;
        }}
        .category-count {{
            margin-left: auto;
            color: var(--text-secondary);
            font-size: 0.875rem;
        }}
        .features-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
            gap: 1rem;
        }}
        .feature-card {{
            background: var(--bg-secondary);
            border-radius: 12px;
            padding: 1rem;
            border: 1px solid transparent;
            transition: all 0.2s ease;
        }}
        .feature-card:hover {{
            border-color: var(--accent-blue);
            transform: translateY(-2px);
        }}
        .feature-card.implemented {{
            border-left: 3px solid var(--accent-green);
        }}
        .feature-card.planned {{
            border-left: 3px solid var(--accent-gray);
            opacity: 0.7;
        }}
        .feature-header {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            margin-bottom: 0.5rem;
        }}
        .status-icon {{
            width: 20px;
            height: 20px;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: 50%;
            font-size: 0.75rem;
        }}
        .implemented .status-icon {{
            background: var(--accent-green);
            color: white;
        }}
        .planned .status-icon {{
            background: var(--accent-gray);
            color: white;
        }}
        .feature-name {{
            font-weight: 600;
            flex: 1;
        }}
        .impl-badge {{
            font-size: 0.7rem;
            padding: 0.2rem 0.5rem;
            border-radius: 4px;
            text-transform: uppercase;
            font-weight: 600;
        }}
        .impl-badge.libzetta {{ background: var(--accent-blue); color: white; }}
        .impl-badge.ffi {{ background: var(--accent-purple); color: white; }}
        .impl-badge.libzfs {{ background: var(--accent-orange); color: white; }}
        .impl-badge.cli {{ background: var(--accent-yellow); color: black; }}
        .impl-badge.hybrid {{ background: linear-gradient(90deg, var(--accent-blue), var(--accent-yellow)); color: black; }}
        .impl-badge.planned {{ background: var(--accent-gray); color: white; }}
        .endpoint {{
            margin-top: 0.5rem;
        }}
        .endpoint code {{
            font-size: 0.8rem;
            background: var(--bg-card);
            padding: 0.25rem 0.5rem;
            border-radius: 4px;
            color: var(--accent-green);
            font-family: 'Monaco', 'Menlo', monospace;
        }}
        .notes {{
            margin-top: 0.5rem;
            font-size: 0.8rem;
            color: var(--text-secondary);
        }}
        .legend {{
            display: flex;
            justify-content: center;
            gap: 1.5rem;
            margin-top: 2rem;
            flex-wrap: wrap;
        }}
        .legend-item {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            font-size: 0.875rem;
            color: var(--text-secondary);
        }}
        .links {{
            margin-top: 2rem;
            text-align: center;
        }}
        .links a {{
            color: var(--accent-blue);
            text-decoration: none;
            margin: 0 1rem;
        }}
        .links a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>ZFS Agent API</h1>
            <p class="subtitle">Feature Coverage &amp; Implementation Status</p>
            <div class="summary">
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div class="stat-label">Total Features</div>
                </div>
                <div class="stat">
                    <div class="stat-value green">{}</div>
                    <div class="stat-label">Implemented</div>
                </div>
                <div class="stat">
                    <div class="stat-value orange">{}</div>
                    <div class="stat-label">Planned</div>
                </div>
            </div>
            <div class="legend">
                <div class="legend-item"><span class="impl-badge libzetta">libzetta</span> Library bindings</div>
                <div class="legend-item"><span class="impl-badge ffi">FFI</span> Direct lzc_* calls</div>
                <div class="legend-item"><span class="impl-badge libzfs">libzfs</span> libzfs FFI</div>
                <div class="legend-item"><span class="impl-badge hybrid">Hybrid</span> libzetta + CLI</div>
                <div class="legend-item"><span class="impl-badge cli">CLI</span> Experimental</div>
            </div>
        </header>

        {}

        <div class="links">
            <a href="/v1/docs">API Documentation</a>
            <a href="/v1/features?format=json">JSON Format</a>
            <a href="/v1/health">Health Check</a>
        </div>
    </div>
</body>
</html>"##,
        data.summary.total, data.summary.implemented, data.summary.planned, features_html
    )
}

// ============================================================================
// Safety Lock Handlers
// ============================================================================

/// GET /v1/safety - Get safety status
/// Always accessible (no auth, works even when locked)
pub async fn safety_status_handler(
    safety_manager: SafetyManager,
) -> Result<impl Reply, Rejection> {
    let state = safety_manager.get_state();

    Ok(warp::reply::json(&SafetyStatusResponse {
        status: "success".to_string(),
        locked: state.locked,
        compatible: state.compatible,
        zfs_version: state.zfs_version,
        agent_version: state.agent_version,
        approved_versions: state.approved_versions,
        lock_reason: state.lock_reason,
        override_at: state.override_at,
    }))
}

/// POST /v1/safety - Override safety lock
/// Always accessible (no auth, works even when locked)
pub async fn safety_override_handler(
    body: SafetyOverrideRequest,
    safety_manager: SafetyManager,
) -> Result<impl Reply, Rejection> {
    if body.action != "override" {
        return Ok(warp::reply::json(&SafetyOverrideResponse {
            status: "error".to_string(),
            message: format!("Unknown action '{}'. Use 'override'.", body.action),
            locked: safety_manager.is_locked(),
        }));
    }

    match safety_manager.override_lock() {
        Ok(_) => Ok(warp::reply::json(&SafetyOverrideResponse {
            status: "success".to_string(),
            message: "Safety lock disabled. All operations now permitted.".to_string(),
            locked: false,
        })),
        Err(e) => Ok(warp::reply::json(&SafetyOverrideResponse {
            status: "error".to_string(),
            message: e,
            locked: safety_manager.is_locked(),
        })),
    }
}

// ============================================================================
// Pool Handlers
// ============================================================================

pub async fn list_pools_handler(zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.list_pools().await {
        Ok(pools) => Ok(success_response(PoolListResponse {
            status: "success".to_string(),
            pools,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list pools: {}", e))),
    }
}

pub async fn get_pool_status_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_pool_status(&name).await {
        Ok(status) => Ok(success_response(PoolStatusResponse {
            status: "success".to_string(),
            name: status.name,
            health: status.health,
            size: status.size,
            allocated: status.allocated,
            free: status.free,
            capacity: status.capacity,
            vdevs: status.vdevs,
            errors: status.errors,
        })),
        Err(e) => Ok(error_response(&format!("Failed to get pool status: {}", e))),
    }
}

pub async fn create_pool_handler(
    body: CreatePool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.create_pool(body).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: "Pool created successfully".to_string(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create pool: {}", e))),
    }
}

pub async fn destroy_pool_handler(
    name: String,
    force: bool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.destroy_pool(&name, force).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' destroyed successfully", name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to destroy pool: {}", e))),
    }
}

pub async fn list_snapshots_handler(
    dataset: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.list_snapshots(&dataset).await {
        Ok(snapshots) => Ok(success_response(ListResponse {
            status: "success".to_string(),
            items: snapshots,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list snapshots: {}", e))),
    }
}

pub async fn create_snapshot_handler(
    dataset: String,
    body: CreateSnapshot,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Validate snapshot name before attempting creation
    if let Err(msg) = validate_snapshot_name(&body.snapshot_name) {
        return Ok(error_response(&format!("Invalid snapshot name: {}", msg)));
    }

    match zfs.create_snapshot(&dataset, &body.snapshot_name).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!(
                "Snapshot '{}@{}' created successfully",
                dataset, body.snapshot_name
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create snapshot: {}", e))),
    }
}

/// Delete snapshot handler that parses path as "dataset/path/snapshot_name"
/// Last segment is the snapshot name, everything before is the dataset path
pub async fn delete_snapshot_by_path_handler(
    path: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    if let Some(pos) = path.rfind('/') {
        let dataset = path[..pos].to_string();
        let snapshot_name = path[pos + 1..].to_string();
        match zfs.delete_snapshot(&dataset, &snapshot_name).await {
            Ok(_) => Ok(success_response(ActionResponse {
                status: "success".to_string(),
                message: format!(
                    "Snapshot '{}@{}' deleted successfully",
                    dataset, snapshot_name
                ),
            })),
            Err(e) => Ok(error_response(&format!("Failed to delete snapshot: {}", e))),
        }
    } else {
        Ok(error_response(
            "Invalid snapshot path: expected /snapshots/dataset/snapshot_name",
        ))
    }
}

pub async fn list_datasets_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.list_datasets(&pool).await {
        Ok(datasets) => Ok(success_response(DatasetResponse {
            status: "success".to_string(),
            datasets,
        })),
        Err(e) => Ok(error_response(&format!("Failed to list datasets: {}", e))),
    }
}

pub async fn create_dataset_handler(
    body: CreateDataset,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Validate dataset name before attempting creation
    if let Err(msg) = validate_dataset_name(&body.name) {
        return Ok(error_response(&format!("Invalid dataset name: {}", msg)));
    }

    match zfs.create_dataset(body).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: "Dataset created successfully".to_string(),
        })),
        Err(e) => Ok(error_response(&format!("Failed to create dataset: {}", e))),
    }
}

pub async fn delete_dataset_handler(
    name: String,
    recursive: bool,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let result = if recursive {
        zfs.delete_dataset_recursive(&name).await
    } else {
        zfs.delete_dataset(&name).await
    };

    match result {
        Ok(_) => {
            let msg = if recursive {
                format!("Dataset '{}' and all children deleted successfully", name)
            } else {
                format!("Dataset '{}' deleted successfully", name)
            };
            Ok(success_response(ActionResponse {
                status: "success".to_string(),
                message: msg,
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to delete dataset: {}", e))),
    }
}

// =========================================================================
// Dataset Properties Handlers
// =========================================================================

/// Get all properties of a dataset
/// libzetta: ZfsEngine::read_properties()
pub async fn get_dataset_properties_handler(
    name: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_dataset_properties(&name).await {
        Ok(props) => Ok(success_response(DatasetPropertiesResponse {
            status: "success".to_string(),
            properties: props,
        })),
        Err(e) => Ok(error_response(&format!(
            "Failed to get dataset properties: {}",
            e
        ))),
    }
}

/// Set a property on a dataset
/// **EXPERIMENTAL**: Uses CLI (`zfs set`) as libzetta/libzfs FFI lacks property setting.
pub async fn set_dataset_property_handler(
    name: String,
    body: SetPropertyRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs
        .set_dataset_property(&name, &body.property, &body.value)
        .await
    {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!(
                "Property '{}' set to '{}' on dataset '{}'",
                body.property, body.value, name
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to set property: {}", e))),
    }
}

// =========================================================================
// Scrub Handlers
// =========================================================================

/// Start a scrub on the pool
pub async fn start_scrub_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.start_scrub(&pool).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Scrub started on pool '{}'", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to start scrub: {}", e))),
    }
}

/// Pause a scrub on the pool
pub async fn pause_scrub_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.pause_scrub(&pool).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Scrub paused on pool '{}'", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to pause scrub: {}", e))),
    }
}

/// Stop a scrub on the pool
pub async fn stop_scrub_handler(pool: String, zfs: ZfsManager) -> Result<impl Reply, Rejection> {
    match zfs.stop_scrub(&pool).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Scrub stopped on pool '{}'", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to stop scrub: {}", e))),
    }
}

/// Get scrub status for the pool
/// Implementation via libzfs FFI bindings.
/// Returns actual scan progress extracted from pool_scan_stat_t.
pub async fn get_scrub_status_handler(
    pool: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.get_scrub_status(&pool).await {
        Ok(scrub) => {
            // Calculate percent_done: (examined / to_examine) * 100
            // Per ZFS docs: check state == finished rather than percent == 100
            let percent_done = match (scrub.examined, scrub.to_examine) {
                (Some(examined), Some(to_examine)) if to_examine > 0 => {
                    Some((examined as f64 / to_examine as f64) * 100.0)
                }
                _ => None,
            };

            Ok(success_response(ScrubStatusResponse {
                status: "success".to_string(),
                pool: pool.clone(),
                pool_health: scrub.pool_health,
                pool_errors: scrub.errors,
                scan_state: scrub.state,
                scan_function: scrub.function,
                start_time: scrub.start_time,
                end_time: scrub.end_time,
                to_examine: scrub.to_examine,
                examined: scrub.examined,
                scan_errors: scrub.scan_errors,
                percent_done,
            }))
        }
        Err(e) => Ok(error_response(&format!(
            "Failed to get scrub status: {}",
            e
        ))),
    }
}

// =========================================================================
// Import/Export Handlers
// =========================================================================

/// Export a pool from the system
pub async fn export_pool_handler(
    pool: String,
    body: ExportPoolRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.export_pool(&pool, body.force).await {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' exported successfully", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to export pool: {}", e))),
    }
}

/// List pools available for import
pub async fn list_importable_pools_handler(
    dir: Option<String>,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    let result = match dir {
        Some(d) => zfs.list_importable_pools_from_dir(&d).await,
        None => zfs.list_importable_pools().await,
    };

    match result {
        Ok(pools) => Ok(success_response(ImportablePoolsResponse {
            status: "success".to_string(),
            pools: pools
                .into_iter()
                .map(|p| ImportablePoolInfo {
                    name: p.name,
                    health: p.health,
                })
                .collect(),
        })),
        Err(e) => Ok(error_response(&format!(
            "Failed to list importable pools: {}",
            e
        ))),
    }
}

/// Import a pool into the system
/// Supports renaming on import via new_name field
pub async fn import_pool_handler(
    body: ImportPoolRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // If new_name is provided, use import_with_name (CLI-based rename)
    let result = match (&body.new_name, &body.dir) {
        (Some(new_name), Some(dir)) => {
            zfs.import_pool_with_name(&body.name, new_name, Some(dir.as_str()))
                .await
        }
        (Some(new_name), None) => zfs.import_pool_with_name(&body.name, new_name, None).await,
        (None, Some(dir)) => zfs.import_pool_from_dir(&body.name, dir).await,
        (None, None) => zfs.import_pool(&body.name).await,
    };

    let imported_name = body.new_name.as_ref().unwrap_or(&body.name);

    match result {
        Ok(_) => Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!("Pool '{}' imported successfully", imported_name),
        })),
        Err(e) => Ok(error_response(&format!("Failed to import pool: {}", e))),
    }
}

// =========================================================================
// Snapshot Clone/Promote Handlers
// =========================================================================

/// Clone a snapshot to create a new writable dataset
/// Implementation via libzetta-zfs-core-sys FFI (lzc_clone)
///
/// Path format: /v1/snapshots/{dataset}/{snapshot}/clone
/// The dataset path can have multiple segments (e.g., tank/data/subdir)
pub async fn clone_snapshot_handler(
    snapshot_path: String, // Full path: dataset/snapshot_name
    body: CloneSnapshotRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path (everything before last '/' is dataset, last segment is snapshot name)
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        match zfs.clone_snapshot(&full_snapshot, &body.target).await {
            Ok(_) => Ok(success_response(CloneResponse {
                status: "success".to_string(),
                origin: full_snapshot,
                clone: body.target,
            })),
            Err(e) => Ok(error_response(&format!("Failed to clone snapshot: {}", e))),
        }
    } else {
        Ok(error_response(
            "Invalid snapshot path: expected /snapshots/dataset/snapshot_name/clone",
        ))
    }
}

/// Promote a clone to an independent dataset
/// Implementation via libzetta-zfs-core-sys FFI (lzc_promote)
///
/// Path format: /v1/datasets/{path}/promote
/// The dataset path can have multiple segments (e.g., tank/data-clone)
pub async fn promote_dataset_handler(
    clone_path: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.promote_dataset(&clone_path).await {
        Ok(_) => Ok(success_response(PromoteResponse {
            status: "success".to_string(),
            dataset: clone_path.clone(),
            message: format!(
                "Dataset '{}' promoted successfully. Former parent is now a clone.",
                clone_path
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to promote dataset: {}", e))),
    }
}

/// Rollback a dataset to a snapshot
/// Implementation via libzetta-zfs-core-sys FFI (lzc_rollback_to)
///
/// Safety levels:
/// - Default: Only allows rollback to most recent snapshot
/// - force_destroy_newer: Destroys intermediate snapshots (like -r)
/// - force_destroy_newer + force_destroy_clones: Also destroys clones (like -R)
///
/// Path format: /v1/datasets/{path}/rollback
pub async fn rollback_dataset_handler(
    dataset: String,
    body: RollbackRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs
        .rollback_dataset(
            &dataset,
            &body.snapshot,
            body.force_destroy_newer,
            body.force_destroy_clones,
        )
        .await
    {
        Ok(result) => Ok(success_response(RollbackResponse {
            status: "success".to_string(),
            dataset: dataset.clone(),
            snapshot: body.snapshot,
            message: format!("Dataset '{}' rolled back successfully", dataset),
            destroyed_snapshots: result.destroyed_snapshots,
            destroyed_clones: result.destroyed_clones,
        })),
        Err(RollbackError::InvalidRequest(msg)) => {
            Ok(error_response(&format!("Invalid request: {}", msg)))
        }
        Err(RollbackError::Blocked {
            message,
            blocking_snapshots,
            blocking_clones,
        }) => {
            // Return structured blocked response with blocking items
            Ok(success_response(RollbackBlockedResponse {
                status: "error".to_string(),
                message,
                blocking_snapshots,
                blocking_clones,
            }))
        }
        Err(RollbackError::ZfsError(msg)) => {
            Ok(error_response(&format!("Rollback failed: {}", msg)))
        }
    }
}

// =========================================================================
// Pool Vdev Operations
// =========================================================================

/// Add a vdev to an existing pool
/// POST /v1/pools/{name}/vdev
///
/// Implementation via libzfs FFI (zpool_add)
/// Supports: disk, mirror, raidz/2/3, log, cache, spare, special, dedup
pub async fn add_vdev_handler(
    pool: String,
    body: AddVdevRequest,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs
        .add_vdev(
            &pool,
            &body.vdev_type,
            body.devices.clone(),
            body.force,
            body.check_ashift,
        )
        .await
    {
        Ok(_) => Ok(success_response(AddVdevResponse {
            status: "success".to_string(),
            pool: pool.clone(),
            vdev_type: body.vdev_type,
            devices: body.devices,
            message: format!("Vdev added to pool '{}' successfully", pool),
        })),
        Err(e) => Ok(error_response(&format!("Failed to add vdev: {}", e))),
    }
}

/// Remove a vdev from an existing pool
/// DELETE /v1/pools/{name}/vdev/{device}
///
/// Implementation via libzfs FFI (zpool_vdev_remove)
/// Can remove: mirrors, single disks, cache, log, spare
/// Cannot remove: raidz, draid (ZFS limitation)
pub async fn remove_vdev_handler(
    pool: String,
    device: String,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    match zfs.remove_vdev(&pool, &device).await {
        Ok(_) => Ok(success_response(RemoveVdevResponse {
            status: "success".to_string(),
            pool: pool.clone(),
            device: device.clone(),
            message: format!(
                "Device '{}' removed from pool '{}' successfully",
                device, pool
            ),
        })),
        Err(e) => Ok(error_response(&format!("Failed to remove vdev: {}", e))),
    }
}

pub async fn execute_command_handler(
    body: CommandRequest,
    last_action: Arc<RwLock<Option<LastAction>>>,
) -> Result<impl Reply, Rejection> {
    // Update last action
    if let Ok(mut action) = last_action.write() {
        *action = Some(LastAction::new("execute_command".to_string()));
    }

    let mut cmd = Command::new(&body.command);

    if let Some(args) = body.args {
        cmd.args(args);
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined_output = format!("{}{}", stdout, stderr);

            Ok(success_response(CommandResponse {
                status: "success".to_string(),
                output: combined_output,
                exit_code: output.status.code().unwrap_or(-1),
            }))
        }
        Err(e) => Ok(error_response(&format!("Failed to execute command: {}", e))),
    }
}

// =========================================================================
// Task Management Handlers
// =========================================================================

/// Get task status by task_id
/// GET /v1/tasks/{task_id}
pub async fn get_task_status_handler(
    task_id: String,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Cleanup expired tasks first
    task_manager.cleanup_expired();

    match task_manager.get_task(&task_id) {
        Some(task) => Ok(success_response(TaskStatusResponse::from(&task))),
        None => Ok(error_response(&format!("Task '{}' not found", task_id))),
    }
}

/// Estimate send stream size for a snapshot
/// GET /v1/snapshots/{dataset}/{snapshot}/send-size
/// Implementation via libzetta-zfs-core-sys FFI (lzc_send_space)
pub async fn send_size_handler(
    snapshot_path: String, // dataset/snapshot_name
    query: SendSizeQuery,
    zfs: ZfsManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        // NOTE: lzc_send_space does NOT support recursive (-R)
        // If recursive is requested, return error
        if query.recursive {
            return Ok(error_response("Recursive (-R) size estimation is not supported by FFI. Estimate individual snapshots."));
        }

        // Determine incremental from snapshot
        let incremental = query.from.is_some();
        let from_snapshot = query.from.as_ref().map(|from| {
            if from.contains('@') {
                from.clone()
            } else {
                format!("{}@{}", dataset, from)
            }
        });

        // Call FFI-based estimate_send_size
        match zfs
            .estimate_send_size(
                &full_snapshot,
                from_snapshot.as_deref(),
                query.raw,
                false, // compressed flag for send_space (not directly supported by lzc_send_space)
            )
            .await
        {
            Ok(estimated_bytes) => {
                let estimated_human = format_bytes(estimated_bytes);

                Ok(success_response(SendSizeResponse {
                    status: "success".to_string(),
                    snapshot: full_snapshot,
                    estimated_bytes,
                    estimated_human,
                    incremental,
                    from_snapshot,
                }))
            }
            Err(e) => Ok(error_response(&e)),
        }
    } else {
        Ok(error_response(
            "Invalid snapshot path: expected /snapshots/dataset/snapshot_name/send-size",
        ))
    }
}

/// Format bytes into human-readable string (e.g., "1.23 GB")
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Send snapshot to file
/// POST /v1/snapshots/{dataset}/{snapshot}/send
pub async fn send_snapshot_handler(
    snapshot_path: String, // dataset/snapshot_name
    body: SendSnapshotRequest,
    zfs: ZfsManager,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        // Check pool busy state first
        let pool = ZfsManager::get_pool_from_path(&full_snapshot);
        if let Some(busy_task) = task_manager.is_pool_busy(&pool) {
            return Ok(error_response(&format!(
                "Pool '{}' is busy with task '{}'",
                pool, busy_task
            )));
        }

        // Dry run just returns estimated size
        if body.dry_run {
            // Use CLI for dry-run size estimation
            let mut args = vec!["send", "-n", "-P"];
            if body.raw {
                args.push("-w");
            }
            if body.recursive {
                args.push("-R");
            }
            args.push(&full_snapshot);

            let output = Command::new("zfs").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut bytes: u64 = 0;
                    for line in stdout.lines() {
                        if line.starts_with("size") {
                            if let Some(s) = line.split_whitespace().nth(1) {
                                bytes = s.parse().unwrap_or(0);
                            }
                        }
                    }
                    return Ok(success_response(SendSizeResponse {
                        status: "success".to_string(),
                        snapshot: full_snapshot,
                        estimated_bytes: bytes,
                        estimated_human: format_bytes(bytes),
                        incremental: body.from_snapshot.is_some(),
                        from_snapshot: body.from_snapshot,
                    }));
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Ok(error_response(&format!(
                        "Dry run failed: {}",
                        stderr.trim()
                    )));
                }
                Err(e) => {
                    return Ok(error_response(&format!("Failed to estimate: {}", e)));
                }
            }
        }

        // Create task
        let task_id = match task_manager
            .create_task(crate::models::TaskOperation::Send, vec![pool.clone()])
        {
            Ok(id) => id,
            Err((pool, task)) => {
                return Ok(error_response(&format!(
                    "Pool '{}' is busy with task '{}'",
                    pool, task
                )));
            }
        };

        // Mark task running
        task_manager.mark_running(&task_id);

        // Execute send operation
        let from_snap = body.from_snapshot.as_deref();
        let result = zfs
            .send_snapshot_to_file(
                &full_snapshot,
                &body.output_file,
                from_snap,
                body.recursive,
                body.properties,
                body.raw,
                body.compressed,
                body.large_blocks,
                body.overwrite,
            )
            .await;

        match result {
            Ok(bytes_written) => {
                task_manager.complete_task(
                    &task_id,
                    serde_json::json!({
                        "bytes_written": bytes_written,
                        "snapshot": full_snapshot,
                        "output_file": body.output_file,
                    }),
                );

                Ok(success_response(TaskResponse {
                    status: "success".to_string(),
                    task_id,
                    message: Some(format!(
                        "Snapshot '{}' sent to '{}' ({} bytes)",
                        full_snapshot, body.output_file, bytes_written
                    )),
                }))
            }
            Err(e) => {
                task_manager.fail_task(&task_id, e.clone());
                Ok(error_response(&e))
            }
        }
    } else {
        Ok(error_response("Invalid snapshot path"))
    }
}

/// Receive snapshot from file
/// POST /v1/datasets/{path}/receive
pub async fn receive_snapshot_handler(
    target_dataset: String,
    body: ReceiveSnapshotRequest,
    zfs: ZfsManager,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Dry run validation only
    if body.dry_run {
        // Validate file exists
        if !std::path::Path::new(&body.input_file).exists() {
            return Ok(error_response(&format!(
                "Input file '{}' does not exist",
                body.input_file
            )));
        }
        return Ok(success_response(ActionResponse {
            status: "success".to_string(),
            message: format!(
                "Dry run: would receive from '{}' to '{}'",
                body.input_file, target_dataset
            ),
        }));
    }

    // Check pool busy state
    let pool = ZfsManager::get_pool_from_path(&target_dataset);
    if let Some(busy_task) = task_manager.is_pool_busy(&pool) {
        return Ok(error_response(&format!(
            "Pool '{}' is busy with task '{}'",
            pool, busy_task
        )));
    }

    // Create task
    let task_id =
        match task_manager.create_task(crate::models::TaskOperation::Receive, vec![pool.clone()]) {
            Ok(id) => id,
            Err((pool, task)) => {
                return Ok(error_response(&format!(
                    "Pool '{}' is busy with task '{}'",
                    pool, task
                )));
            }
        };

    // Mark task running
    task_manager.mark_running(&task_id);

    // Execute receive operation
    let result = zfs
        .receive_snapshot_from_file(&target_dataset, &body.input_file, body.force)
        .await;

    match result {
        Ok(output) => {
            task_manager.complete_task(
                &task_id,
                serde_json::json!({
                    "target_dataset": target_dataset,
                    "input_file": body.input_file,
                    "output": output,
                }),
            );

            Ok(success_response(TaskResponse {
                status: "success".to_string(),
                task_id,
                message: Some(format!(
                    "Received to dataset '{}' from '{}'",
                    target_dataset, body.input_file
                )),
            }))
        }
        Err(e) => {
            task_manager.fail_task(&task_id, e.clone());
            Ok(error_response(&e))
        }
    }
}

/// Replicate snapshot directly to another pool (pipe send→receive)
/// POST /v1/snapshots/{dataset}/{snapshot}/replicate
///
/// IMPORTANT: Both source AND target pools are marked busy during replication
pub async fn replicate_snapshot_handler(
    snapshot_path: String, // dataset/snapshot_name
    body: ReplicateSnapshotRequest,
    zfs: ZfsManager,
    task_manager: TaskManager,
) -> Result<impl Reply, Rejection> {
    // Parse snapshot path
    if let Some(pos) = snapshot_path.rfind('/') {
        let dataset = &snapshot_path[..pos];
        let snapshot_name = &snapshot_path[pos + 1..];
        let full_snapshot = format!("{}@{}", dataset, snapshot_name);

        // Get pools for both source and target (BOTH must be free)
        let source_pool = ZfsManager::get_pool_from_path(&full_snapshot);
        let target_pool = ZfsManager::get_pool_from_path(&body.target_dataset);

        // Check source pool busy state
        if let Some(busy_task) = task_manager.is_pool_busy(&source_pool) {
            return Ok(error_response(&format!(
                "Source pool '{}' is busy with task '{}'",
                source_pool, busy_task
            )));
        }

        // Check target pool busy state (if different from source)
        if source_pool != target_pool {
            if let Some(busy_task) = task_manager.is_pool_busy(&target_pool) {
                return Ok(error_response(&format!(
                    "Target pool '{}' is busy with task '{}'",
                    target_pool, busy_task
                )));
            }
        }

        // Dry run returns estimated size
        if body.dry_run {
            let mut args = vec!["send", "-n", "-P"];
            if body.raw {
                args.push("-w");
            }
            if body.recursive {
                args.push("-R");
            }
            args.push(&full_snapshot);

            let output = Command::new("zfs").args(&args).output();
            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut bytes: u64 = 0;
                    for line in stdout.lines() {
                        if line.starts_with("size") {
                            if let Some(s) = line.split_whitespace().nth(1) {
                                bytes = s.parse().unwrap_or(0);
                            }
                        }
                    }
                    return Ok(success_response(SendSizeResponse {
                        status: "success".to_string(),
                        snapshot: full_snapshot,
                        estimated_bytes: bytes,
                        estimated_human: format_bytes(bytes),
                        incremental: body.from_snapshot.is_some(),
                        from_snapshot: body.from_snapshot,
                    }));
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Ok(error_response(&format!(
                        "Dry run failed: {}",
                        stderr.trim()
                    )));
                }
                Err(e) => {
                    return Ok(error_response(&format!("Failed to estimate: {}", e)));
                }
            }
        }

        // Mark BOTH pools as busy (critical for replication)
        let pools = if source_pool != target_pool {
            vec![source_pool.clone(), target_pool.clone()]
        } else {
            vec![source_pool.clone()]
        };

        // Create task
        let task_id = match task_manager.create_task(crate::models::TaskOperation::Replicate, pools)
        {
            Ok(id) => id,
            Err((pool, task)) => {
                return Ok(error_response(&format!(
                    "Pool '{}' is busy with task '{}'",
                    pool, task
                )));
            }
        };

        // Mark task running
        task_manager.mark_running(&task_id);

        // Execute replication
        let from_snap = body.from_snapshot.as_deref();
        let result = zfs
            .replicate_snapshot(
                &full_snapshot,
                &body.target_dataset,
                from_snap,
                body.recursive,
                body.properties,
                body.raw,
                body.compressed,
                body.force,
            )
            .await;

        match result {
            Ok(output) => {
                task_manager.complete_task(
                    &task_id,
                    serde_json::json!({
                        "source": full_snapshot,
                        "target": body.target_dataset,
                        "output": output,
                    }),
                );

                Ok(success_response(TaskResponse {
                    status: "success".to_string(),
                    task_id,
                    message: Some(format!(
                        "Replicated '{}' to '{}'",
                        full_snapshot, body.target_dataset
                    )),
                }))
            }
            Err(e) => {
                task_manager.fail_task(&task_id, e.clone());
                Ok(error_response(&e))
            }
        }
    } else {
        Ok(error_response("Invalid snapshot path"))
    }
}
