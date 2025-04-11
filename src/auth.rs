//-----------------------------------------------------
// AUTHENTICATION
//-----------------------------------------------------

use std::fs;
use warp::http::HeaderMap;

// Custom error type for API key validation failures
#[derive(Debug)]
struct ApiKeyError;
impl warp::reject::Reject for ApiKeyError {}

// Function to get an existing API key or create a new one
fn get_or_create_api_key() -> Result<String, Box<dyn std::error::Error>> {
    let file_path = ".zfswm_api";
    if let Ok(api_key) = fs::read_to_string(file_path) {
        Ok(api_key.trim().to_string())
    } else {
        let api_key: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let mut file = fs::File::create(file_path)?;
        file.write_all(api_key.as_bytes())?;
        Ok(api_key)
    }
}

// Check if the API key is valid
async fn check_api_key(headers: HeaderMap, our_api_key: String) -> Result<(), Rejection> {
    match headers.get("X-API-Key") {
        Some(key) if key.to_str().map(|s| s == our_api_key).unwrap_or(false) => Ok(()),
        _ => Err(warp::reject::custom(ApiKeyError)),
    }
}