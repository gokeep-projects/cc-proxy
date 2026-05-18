pub mod convert;
pub mod handler;

use axum::routing::any;
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/chat/completions", any(handler::proxy_handler))
        .route("/v1/responses", any(handler::responses_handler))
        .route("/v1/messages", any(handler::messages_handler))
        .route("/v1/models", any(handler::models_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
