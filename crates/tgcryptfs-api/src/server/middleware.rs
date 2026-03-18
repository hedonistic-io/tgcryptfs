use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Request logging middleware.
pub async fn request_logger(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    tracing::info!(%method, %uri, "request started");

    let response = next.run(request).await;

    let elapsed = start.elapsed();
    let status = response.status();
    tracing::info!(%method, %uri, %status, elapsed_ms = elapsed.as_millis(), "request completed");

    response
}
