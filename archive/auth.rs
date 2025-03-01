use warp::{Filter, Rejection, http::HeaderMap};

// Custom error type for API key validation failures
#[derive(Debug)]
pub struct ApiKeyError;
impl warp::reject::Reject for ApiKeyError {}

// API key validation filter
pub fn api_key_filter(api_key: String) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    let key = api_key.clone();
    warp::header::<String>("X-API-Key")
        .and_then(move |header_key: String| {
            let our_key = key.clone();
            async move {
                if header_key == our_key {
                    Ok(())
                } else {
                    Err(warp::reject::custom(ApiKeyError))
                }
            }
        })
        .map(|_| ()) // Explicitly map to unit type
}

// Check if the API key is valid
async fn check_api_key(headers: HeaderMap, our_api_key: String) -> Result<(), Rejection> {
    match headers.get("X-API-Key") {
        Some(key) if key.to_str().map(|s| s == our_api_key).unwrap_or(false) => Ok(()),
        _ => Err(warp::reject::custom(ApiKeyError)),
    }
}