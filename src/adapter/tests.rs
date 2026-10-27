use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use lemonfiber_contract::adapter::{about_path, ready_path, About};
use serde_json::json;
use tower::ServiceExt as _;

use super::{about, kit, DIGEST, RELEASE, UPSTREAM};
use crate::caddy::Caddy;
use crate::fake::{serve, Held, Socket};

fn a_key() -> String {
    ('e'..='t').collect()
}

async fn answered(
    caddy: Arc<Caddy>,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    let Ok(request) = request.body(Body::from(body.to_owned())) else {
        return (StatusCode::IM_A_TEAPOT, Vec::new());
    };
    let Ok(response) = kit(a_key(), caddy).router().oneshot(request).await;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, body)
}

#[test]
fn it_says_it_speaks_proxy_front_for_the_caddy_its_recordings_were_taken_from() {
    let about = about();

    assert_eq!(about.speaks, vec!["proxy.front@1".to_owned()]);
    assert_eq!(about.upstream, UPSTREAM);
    assert_eq!(about.releases.len(), 1);
    assert_eq!(
        about.releases.first().map(|one| one.version.as_str()),
        Some(RELEASE)
    );
    assert_eq!(
        about.releases.first().map(|one| one.digest.as_str()),
        Some(DIGEST)
    );
}

#[tokio::test]
async fn it_names_itself_under_the_key_and_to_nobody_else() {
    let socket = Socket::new("adapter-about");
    let caddy = Arc::new(Caddy::at(&socket.0));

    let (status, body) =
        answered(Arc::clone(&caddy), "GET", &about_path(), Some(&a_key()), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<About>(&body).ok(), Some(about()));

    let (status, _) = answered(caddy, "GET", &about_path(), Some("another"), "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn it_is_ready_while_caddy_answers_its_admin_api() {
    let socket = Socket::new("adapter-ready");
    let caddy = Arc::new(Caddy::at(&socket.0));
    let (_, before) = answered(Arc::clone(&caddy), "GET", &ready_path(), Some(&a_key()), "").await;

    assert!(serve(&socket.0, Arc::new(Mutex::new(Held::default()))));
    let (_, after) = answered(caddy, "GET", &ready_path(), Some(&a_key()), "").await;

    assert_eq!(before, b"false");
    assert_eq!(after, b"true");
}

#[tokio::test]
async fn a_table_applied_over_the_contract_reaches_caddy() {
    let socket = Socket::new("adapter-apply");
    let held = Arc::new(Mutex::new(Held::default()));
    assert!(serve(&socket.0, Arc::clone(&held)));
    let caddy = Arc::new(Caddy::at(&socket.0));
    let table = json!({"table": {"tls": "automatic", "routes": [
        {"name": "watch.home.example", "host": "door", "port": 8096}
    ]}});

    let (status, _) = answered(
        caddy,
        "POST",
        "/lemonfiber/proxy.front/v1/apply",
        Some(&a_key()),
        &table.to_string(),
    )
    .await;

    assert!(status.is_success(), "{status}");
    assert_eq!(
        held.lock().map(|held| held.loads.len()).unwrap_or_default(),
        1
    );
}
