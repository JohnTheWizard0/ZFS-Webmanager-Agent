//! # Integration Tests: Authentication
//!
//! **Modules:** MI-001 (Auth)
//! **Dependencies:** MI-002 (API Framework)
//!
//! ## What is tested
//! - Requests without X-API-Key header → 401 Unauthorized
//! - Requests with wrong X-API-Key → 401 Unauthorized
//! - Requests with correct X-API-Key → proceeds to handler
//!
//! ## Expected outcome
//! - Protected endpoints reject unauthenticated requests
//! - Valid API key grants access
//!
//! ## Prerequisites
//! - Server running on localhost:9876
//! - Valid API key available
//!
//! ## BLOCKED
//! - Cannot run until crate compiles (zfs_management.rs bugs)

/*
use warp::test::request;
use warp::http::StatusCode;

#[tokio::test]
async fn test_missing_api_key_returns_401() {
    // TODO: Request to /pools without X-API-Key
    // let resp = request()
    //     .method("GET")
    //     .path("/pools")
    //     .reply(&routes)
    //     .await;
    // assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_api_key_returns_401() {
    // TODO: Request with wrong API key
    // let resp = request()
    //     .method("GET")
    //     .path("/pools")
    //     .header("X-API-Key", "wrong-key")
    //     .reply(&routes)
    //     .await;
    // assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_valid_api_key_proceeds() {
    // TODO: Request with correct API key
    // Should return 200 (or appropriate response, not 401)
}
*/

#[test]
#[ignore = "Integration test requires running server - use zfs_parcour.sh"]
fn placeholder_auth_tests() {
    // Real auth integration tests run via tests/zfs_parcour.sh
}
