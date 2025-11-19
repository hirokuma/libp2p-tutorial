use std::collections::HashMap;

use anyhow::Result;
use axum::{
    Json,
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::json;

use crate::{CommandHandler, RestReq, RestRes, cmd};

pub async fn start(host: String) {
    // Build our application with some routes
    let app = Router::new()
        .route("/", post(AppState::handler))
        .with_state(AppState::new());

    // Run our application
    let listener = tokio::net::TcpListener::bind(host)
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    handlers: HashMap<&'static str, CommandHandler>,
}

impl AppState {
    fn new() -> Self {
        let handlers = cmd::register_handle();

        Self { handlers }
    }

    // "/" に対してここが post で呼ばれる
    async fn handler(
        State(state): State<Self>,
        Json(value): Json<RestReq>,
    ) -> Result<Json<RestRes>, AppError> {
        match state.handlers.get(value.command.as_str()) {
            Some(func) => {
                let res = func(&value)?;
                Ok(Json(res))
            },
            None => Err(AppError(anyhow::anyhow!("だめ"))),
        }
    }
}

// https://github.com/tokio-rs/axum/blob/b1ef45469bf8ffa334e86ddd12e7f4d4b82fa1ab/examples/anyhow-error-response/src/main.rs
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let payload = json!({
            "message": "not success",
            "from": self.0.to_string(),
        });
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(payload),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
