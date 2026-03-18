pub mod auth;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod state;
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers;

use std::net::SocketAddr;
use std::path::PathBuf;

use http::header;
use http::Method;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::service::auth::AuthService;

use self::auth::BearerAuth;
use self::routes::api_router;
use self::state::AppState;

/// Configuration for the API server.
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub volumes_dir: PathBuf,
    pub auth: AuthService,
    /// Optional pre-set bearer token (for testing). If None, one is generated.
    pub bearer_token: Option<String>,
}

/// Build the axum application with all middleware and routes.
pub fn build_app(state: AppState, bearer_auth: BearerAuth) -> axum::Router {
    // Restrictive CORS: localhost-only origins, specific methods and headers
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            "http://localhost:3000".parse().unwrap(),
            "http://localhost:8080".parse().unwrap(),
            "http://127.0.0.1:3000".parse().unwrap(),
            "http://127.0.0.1:8080".parse().unwrap(),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .max_age(std::time::Duration::from_secs(3600));

    api_router(state, bearer_auth)
        .layer(axum::middleware::from_fn(middleware::request_logger))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

/// Start the API server. This function blocks until the server shuts down.
pub async fn run_server(config: ServerConfig) -> std::io::Result<()> {
    // Generate or use provided bearer token
    let token = config.bearer_token.unwrap_or_else(auth::generate_token);
    let bearer_auth = BearerAuth::new(&token);

    let state = AppState::new(config.volumes_dir, config.auth);
    let app = build_app(state, bearer_auth);

    tracing::info!(addr = %config.bind_addr, "starting API server");

    // Display the bearer token to the user so they can authenticate
    eprintln!();
    eprintln!("API bearer token (include in Authorization header):");
    eprintln!("  {token}");
    eprintln!();
    eprintln!("Example:");
    eprintln!(
        "  curl -H 'Authorization: Bearer {token}' http://{}/api/v1/volumes",
        config.bind_addr
    );
    eprintln!();

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await
}
