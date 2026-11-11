use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use bastion_core::{CoreError, CoreService};
use bastion_mcp_core::QuoteMailConfirmation;
use serde::Deserialize;

use crate::consent_http::{browser_session, error, response};

#[cfg(test)]
#[path = "quote_mail_http_tests.rs"]
mod tests;

#[derive(Clone)]
pub(crate) struct QuoteMailState {
    pub core: CoreService,
    pub origin: String,
    pub mail: QuoteMailConfirmation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    handle: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    handle: String,
    decision: Choice,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Choice {
    Approve,
    Deny,
}

pub(crate) fn quote_mail_router(state: QuoteMailState) -> Router {
    Router::new()
        .route("/oauth/quote-email/review", post(review))
        .route("/oauth/quote-email/decision", post(decide))
        .layer(DefaultBodyLimit::max(4096))
        .with_state(state)
}

fn result(value: Result<serde_json::Value, CoreError>) -> Response {
    match value {
        Ok(value) => response(StatusCode::OK, value),
        Err(reason) => error(match reason {
            CoreError::Unauthorized | CoreError::SessionReplaced => StatusCode::UNAUTHORIZED,
            CoreError::Forbidden => StatusCode::FORBIDDEN,
            CoreError::NotFound => StatusCode::NOT_FOUND,
            CoreError::Conflict | CoreError::PreconditionFailed => StatusCode::CONFLICT,
            CoreError::Invalid => StatusCode::BAD_REQUEST,
            _ => StatusCode::SERVICE_UNAVAILABLE,
        }),
    }
}

async fn review(
    State(state): State<QuoteMailState>,
    headers: HeaderMap,
    Json(input): Json<Review>,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(session) => session,
        Err(status) => return error(status),
    };
    result(state.mail.review(&session, &input.handle).await)
}

async fn decide(
    State(state): State<QuoteMailState>,
    headers: HeaderMap,
    Json(input): Json<Decision>,
) -> Response {
    let session = match browser_session(&state.core, &state.origin, &headers).await {
        Ok(session) => session,
        Err(status) => return error(status),
    };
    result(
        state
            .mail
            .decide(
                &session,
                &input.handle,
                matches!(input.decision, Choice::Approve),
            )
            .await,
    )
}
