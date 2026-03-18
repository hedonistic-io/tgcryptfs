use axum::routing::{delete, get, post};
use axum::Router;

use super::auth::BearerAuth;
use super::handlers;
use super::state::AppState;

/// Build the complete API router with all v1 routes.
pub fn api_router(state: AppState, bearer_auth: BearerAuth) -> Router {
    let v1 = Router::new()
        // System
        .route("/status", get(handlers::system::status))
        .route("/version", get(handlers::system::version))
        // Auth
        .route("/auth/session", post(handlers::auth::login_start))
        .route("/auth/session", delete(handlers::auth::logout))
        .route("/auth/status", get(handlers::auth::status))
        // Volumes
        .route("/volumes", post(handlers::volume::create))
        .route("/volumes", get(handlers::volume::list))
        .route("/volumes/:id", get(handlers::volume::info))
        .route("/volumes/:id", delete(handlers::volume::delete))
        .route("/volumes/:id/open", post(handlers::volume::open))
        .route("/volumes/:id/close", post(handlers::volume::close))
        .route("/volumes/:id/mount", post(handlers::volume::mount))
        .route("/volumes/:id/unmount", post(handlers::volume::unmount))
        // Sharing
        .route("/shares/volume/:volume_id", get(handlers::sharing::list))
        .route("/shares", post(handlers::sharing::create))
        .route("/shares/:id", delete(handlers::sharing::revoke))
        // Invites
        .route("/invites", post(handlers::sharing::create_invite))
        .route(
            "/invites/:code/accept",
            post(handlers::sharing::accept_invite),
        )
        // Deadman
        .route("/deadman/status", get(handlers::deadman::status))
        .route("/deadman/arm", post(handlers::deadman::arm))
        .route("/deadman/disarm", post(handlers::deadman::disarm));

    // Auth middleware applied as a route_layer so it runs after Extension is available.
    // Extension is added first (outermost), then auth middleware runs (innermost).
    Router::new()
        .nest("/api/v1", v1)
        .route_layer(axum::middleware::from_fn(super::auth::require_auth))
        .layer(axum::Extension(bearer_auth))
        .with_state(state)
}
