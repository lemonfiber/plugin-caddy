//! A Caddy admin API on a unix socket, holding the configuration loaded into it and every
//! load it was asked for.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;

/// What the fake holds: the configuration, every load, the status it refuses with, and
/// whether it runs another configuration than the one loaded.
#[derive(Default)]
pub(crate) struct Held {
    pub(crate) config: Value,
    pub(crate) loads: Vec<(Value, bool)>,
    pub(crate) refusing: Option<u16>,
    pub(crate) altering: bool,
}

/// A socket of its own in the system's temporary directory, removed when dropped.
pub(crate) struct Socket(pub(crate) PathBuf);

impl Socket {
    pub(crate) fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("caddy-{name}-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Self(path)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The admin API, answering on `socket` from what `held` holds, and whether it does.
pub(crate) fn serve(socket: &Path, held: Arc<Mutex<Held>>) -> bool {
    std::os::unix::net::UnixListener::bind(socket)
        .ok()
        .filter(|listener| listener.set_nonblocking(true).is_ok())
        .and_then(|listener| tokio::net::UnixListener::from_std(listener).ok())
        .map(|listener| {
            let app = Router::new()
                .route("/config/", get(config))
                .route("/load", post(load))
                .with_state(held);
            tokio::spawn(async move { axum::serve(listener, app).await });
        })
        .is_some()
}

async fn config(State(held): State<Arc<Mutex<Held>>>) -> Result<Json<Value>, StatusCode> {
    let held = held.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    held.refusing.map_or_else(
        || Ok(Json(held.config.clone())),
        |status| Err(refusal(status)),
    )
}

async fn load(
    State(held): State<Arc<Mutex<Held>>>,
    headers: HeaderMap,
    Json(config): Json<Value>,
) -> StatusCode {
    let again = headers
        .get("cache-control")
        .is_some_and(|value| value == "must-revalidate");
    held.lock()
        .map_or(StatusCode::INTERNAL_SERVER_ERROR, |mut held| {
            held.refusing.map_or_else(
                || {
                    held.loads.push((config.clone(), again));
                    held.config = if held.altering {
                        serde_json::json!({"apps": {"http": {"servers": {"lemonfiber": {"routes": []}}}}})
                    } else {
                        config
                    };
                    StatusCode::OK
                },
                refusal,
            )
        })
}

/// The status a refusing fake answers with.
fn refusal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST)
}
