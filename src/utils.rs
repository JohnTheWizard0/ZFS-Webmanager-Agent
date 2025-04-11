//-----------------------------------------------------
// HELPER FUNCTIONS
//-----------------------------------------------------

// Helper functions for response generation
fn success_response(message: &str) -> ActionResponse {
    ActionResponse {
        status: "success".to_string(),
        message: message.to_string(),
    }
}

fn error_response(error: &dyn std::error::Error) -> ActionResponse {
    ActionResponse {
        status: "error".to_string(),
        message: error.to_string(),
    }
}

// Create a middleware filter that tracks actions
fn with_action_tracking(
    action_name: &'static str,
    action_tracker: Arc<RwLock<Option<LastAction>>>,
) -> impl Filter<Extract = (), Error = Infallible> + Clone {
    // Clone the Arc outside the closure so it's moved into the filter
    let tracker = action_tracker.clone();
    
    warp::any()
        .map(move || {
            // Now use the tracker that was cloned outside the closure
            if let Ok(mut last_action) = tracker.write() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                *last_action = Some(LastAction {
                    function: action_name.to_string(),
                    timestamp: now,
                });
            }
        })
        .untuple_one()
}

// Add this function to execute Linux commands
fn execute_linux_command(command: &str, args: &[&str]) -> Result<(String, Option<i32>), std::io::Error> {
    // Create a new Command instance
    let output = Command::new(command)
        .args(args)
        .output()?;
    
    // Combine stdout and stderr
    let mut result = String::new();
    
    // Add stdout if not empty
    if !output.stdout.is_empty() {
        result.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    
    // Add stderr if not empty
    if !output.stderr.is_empty() {
        if !result.is_empty() {
            result.push_str("\n\nSTDERR:\n");
        }
        result.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    
    // Return the output and exit code
    Ok((result, output.status.code()))
}
