use std::fs;
use std::path::PathBuf;
use warp::{Rejection, http::HeaderMap};
use uuid::Uuid;

const API_KEY_FILE: &str = "api_key.txt";

pub fn get_or_create_api_key() -> Result<String, Box<dyn std::error::Error>> {
    let mut api_key_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    api_key_path.push("zfs_webmanager");
    
    // Create directory if it doesn't exist
    if !api_key_path.exists() {
        fs::create_dir_all(&api_key_path)?;
    }
    
    api_key_path.push(API_KEY_FILE);

    if api_key_path.exists() {
        // Read existing API key
        let api_key = fs::read_to_string(&api_key_path)?
            .trim()
            .to_string();
        
        if !api_key.is_empty() {
            return Ok(api_key);
        }
    }

    // Generate new API key
    let api_key = Uuid::new_v4().to_string();
    fs::write(&api_key_path, &api_key)?;
    
    Ok(api_key)
}

pub async fn check_api_key(
    headers: HeaderMap,
    expected_api_key: String,
) -> Result<(), Rejection> {
    match headers.get("X-API-Key") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(provided_key) => {
                    if provided_key == expected_api_key {
                        Ok(())
                    } else {
                        Err(warp::reject::custom(ApiKeyError::Invalid))
                    }
                }
                Err(_) => Err(warp::reject::custom(ApiKeyError::Invalid)),
            }
        }
        None => Err(warp::reject::custom(ApiKeyError::Missing)),
    }
}

#[derive(Debug)]
pub enum ApiKeyError {
    Missing,
    Invalid,
}

impl warp::reject::Reject for ApiKeyError {}

// ============================================================================
// UNIT TESTS — MI-001 (Auth)
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use warp::http::HeaderValue;

    /// Test: Generated API key is valid UUID v4 format
    /// Expected: 36 chars, 4 hyphens, lowercase hex
    #[test]
    fn test_uuid_format() {
        let uuid = Uuid::new_v4().to_string();
        assert_eq!(uuid.len(), 36, "UUID should be 36 characters");
        assert_eq!(uuid.matches('-').count(), 4, "UUID should have 4 hyphens");
        assert!(uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "UUID should only contain hex digits and hyphens");
    }

    /// Test: API key validation - correct key accepted
    /// Expected: Ok(())
    #[tokio::test]
    async fn test_check_api_key_valid() {
        let expected = "test-api-key-12345".to_string();
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("test-api-key-12345"));

        let result = check_api_key(headers, expected).await;
        assert!(result.is_ok(), "Valid API key should be accepted");
    }

    /// Test: API key validation - wrong key rejected
    /// Expected: Err(ApiKeyError::Invalid)
    #[tokio::test]
    async fn test_check_api_key_invalid() {
        let expected = "correct-key".to_string();
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("wrong-key"));

        let result = check_api_key(headers, expected).await;
        assert!(result.is_err(), "Invalid API key should be rejected");
    }

    /// Test: API key validation - missing header rejected
    /// Expected: Err(ApiKeyError::Missing)
    #[tokio::test]
    async fn test_check_api_key_missing() {
        let expected = "some-key".to_string();
        let headers = HeaderMap::new(); // No X-API-Key header

        let result = check_api_key(headers, expected).await;
        assert!(result.is_err(), "Missing API key should be rejected");
    }

    /// Test: ApiKeyError variants exist and are distinct
    /// Expected: Debug output differs for each variant
    #[test]
    fn test_api_key_error_variants() {
        let missing = format!("{:?}", ApiKeyError::Missing);
        let invalid = format!("{:?}", ApiKeyError::Invalid);
        assert_ne!(missing, invalid, "Error variants should be distinct");
        assert!(missing.contains("Missing"));
        assert!(invalid.contains("Invalid"));
    }
}