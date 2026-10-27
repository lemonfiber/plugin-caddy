use std::sync::{Arc, Mutex};

use lemonfiber_ports::service::{Failure, Proxying, Route, Routing, Tls};
use serde_json::json;

use super::Caddy;
use crate::fake::{serve, Held, Socket};

fn table() -> Routing {
    Routing {
        tls: Tls::Automatic,
        routes: vec![Route {
            name: "watch.home.example".to_owned(),
            host: "door".to_owned(),
            port: 8096,
        }],
    }
}

fn caddy(name: &str) -> (Socket, Arc<Mutex<Held>>, Caddy) {
    let socket = Socket::new(name);
    let held = Arc::new(Mutex::new(Held::default()));
    let _ = serve(&socket.0, Arc::clone(&held));
    let caddy = Caddy::at(&socket.0);
    (socket, held, caddy)
}

#[tokio::test]
async fn a_table_applied_is_loaded_once_and_served() {
    let (_socket, held, caddy) = caddy("applied");

    assert!(caddy.apply(&table()).await.is_ok());

    assert_eq!(caddy.routing().await.ok(), Some(table()));
    let loads = held
        .lock()
        .map(|held| held.loads.clone())
        .unwrap_or_default();
    assert_eq!(loads.len(), 1);
    assert!(loads.iter().all(|(_, again)| !again));
}

#[tokio::test]
async fn a_reload_loads_the_configuration_caddy_runs_again() {
    let (_socket, held, caddy) = caddy("reloaded");
    assert!(caddy.apply(&table()).await.is_ok());

    assert!(caddy.reload().await.is_ok());

    let loads = held
        .lock()
        .map(|held| held.loads.clone())
        .unwrap_or_default();
    assert_eq!(loads.len(), 2);
    assert_eq!(loads.get(1).map(|(_, again)| *again), Some(true));
    assert_eq!(
        loads.first().map(|(config, _)| config.clone()),
        loads.get(1).map(|(config, _)| config.clone())
    );
}

#[tokio::test]
async fn a_table_that_reads_back_as_another_is_refused() {
    let (_socket, held, caddy) = caddy("altered");
    if let Ok(mut held) = held.lock() {
        held.altering = true;
    }

    assert!(matches!(
        caddy.apply(&table()).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_name_that_is_not_plain_is_refused_before_caddy_is_asked() {
    let (_socket, held, caddy) = caddy("unnamed");
    let mut unnamed = table();
    unnamed.routes = vec![Route {
        name: "a b".to_owned(),
        host: "door".to_owned(),
        port: 8096,
    }];

    assert!(matches!(
        caddy.apply(&unnamed).await,
        Err(Failure::Refused { .. })
    ));
    assert!(held.lock().is_ok_and(|held| held.loads.is_empty()));
}

#[tokio::test]
async fn a_configuration_no_table_was_loaded_as_is_refused() {
    let (_socket, held, caddy) = caddy("unread");
    if let Ok(mut held) = held.lock() {
        held.config = json!({"apps": {"http": {"servers": {"lemonfiber": {"routes": [{}]}}}}});
    }

    assert!(matches!(
        caddy.routing().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_answer_too_large_to_be_a_configuration_is_refused() {
    let (_socket, held, caddy) = caddy("large");
    if let Ok(mut held) = held.lock() {
        held.config = json!("x".repeat(5 * 1024 * 1024));
    }

    assert!(matches!(
        caddy.routing().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_table_caddy_refuses_or_one_it_would_be_handed_outside_its_directory_is_refused() {
    let (_socket, held, caddy) = caddy("refusing");
    let outside = Routing {
        tls: Tls::Files {
            certificate: "/etc/shadow".to_owned(),
            key: "house.key".to_owned(),
        },
        routes: Vec::new(),
    };
    assert!(matches!(
        caddy.apply(&outside).await,
        Err(Failure::Refused { .. })
    ));
    assert!(held.lock().is_ok_and(|held| held.loads.is_empty()));

    if let Ok(mut held) = held.lock() {
        held.refusing = Some(400);
    }
    assert!(matches!(
        caddy.apply(&table()).await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        caddy.routing().await,
        Err(Failure::Refused { .. })
    ));
    assert!(!caddy.answers().await);
}

#[tokio::test]
async fn a_caddy_whose_socket_answers_nothing_is_unavailable() {
    let socket = Socket::new("silent");
    let caddy = Caddy::at(&socket.0);

    assert!(matches!(
        caddy.routing().await,
        Err(Failure::Unavailable { .. })
    ));
    assert!(!caddy.answers().await);
}

#[tokio::test]
async fn an_answer_that_is_not_a_configuration_is_refused() {
    let socket = Socket::new("garbled");
    let Ok(listener) = std::os::unix::net::UnixListener::bind(&socket.0) else {
        return;
    };
    let _ = listener.set_nonblocking(true);
    let Ok(listener) = tokio::net::UnixListener::from_std(listener) else {
        return;
    };
    let app = axum::Router::new().route("/config/", axum::routing::get(|| async { "not json" }));
    tokio::spawn(async move { axum::serve(listener, app).await });

    assert!(matches!(
        Caddy::at(&socket.0).routing().await,
        Err(Failure::Refused { .. })
    ));
}
