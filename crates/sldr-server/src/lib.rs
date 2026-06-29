//! sldr-server - HTTP API for sldr
//!
//! Two ways to serve, matching ADR-0009 (studio is an API-first satellite;
//! Oqto is a client):
//!
//! - [`router`] — the raw API routes, no auth. Mount this into a host that
//!   already authenticates (e.g. Oqto's backend behind its own middleware).
//! - [`app`] — the standalone application: `router` plus an unauthenticated
//!   `/health` probe and an optional Bearer-token guard. Used by the
//!   `sldr-server` binary on a Tailscale-only deployment.

pub mod models;
pub mod preview;
pub mod routes;
pub mod state;

use axum::extract::{Request, State};
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};

pub use routes::router;
pub use state::SldrState;

/// Standalone serve configuration.
#[derive(Clone, Default)]
pub struct ServeOptions {
    /// Bearer token required on every API route. `None` disables auth — only
    /// appropriate for a trusted local/dev box. Oqto's `authFetch` sends this
    /// as `Authorization: Bearer <token>`, so the studio plugs in with no
    /// adapter (ADR-0009; light token, Tailscale-only).
    pub token: Option<String>,
}

/// Build the standalone application: the API, an open `/health`, and the
/// Bearer-token guard when a token is configured.
pub fn app(state: SldrState, opts: ServeOptions) -> Router {
    let mut api = router(state);
    if let Some(token) = opts.token {
        api = api.layer(middleware::from_fn_with_state(
            AuthToken(token),
            require_bearer,
        ));
    }
    // `/health` is intentionally outside the auth layer so a monitor (or Oqto)
    // can probe liveness without the token.
    Router::new().route("/health", get(health)).merge(api)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "service": "sldr-server",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Clone)]
struct AuthToken(String);

/// Require `Authorization: Bearer <token>` to match the configured token.
/// A plain comparison — sufficient for a light, Tailscale-only token; this is
/// explicitly not a hardened public auth story (ADR-0009).
async fn require_bearer(
    State(expected): State<AuthToken>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let presented = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    match presented {
        Some(token) if token == expected.0 => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
