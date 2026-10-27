use std::sync::{Arc, Mutex};

use serde_json::json;

use super::{Admin, Answer, Call, Unanswered};
use crate::fake::{serve, Held, Socket};

fn get(path: &str) -> Call<'_> {
    Call {
        method: "GET",
        path,
        json: None,
        again: false,
    }
}

#[tokio::test]
async fn a_call_is_answered_over_the_socket() {
    let socket = Socket::new("admin-answered");
    let held = Arc::new(Mutex::new(Held {
        config: json!({"apps": {}}),
        ..Held::default()
    }));
    assert!(serve(&socket.0, held));

    let answer = Admin::at(&socket.0).ask(get("/config/")).await;

    assert_eq!(
        answer,
        Ok(Answer {
            status: 200,
            body: br#"{"apps":{}}"#.to_vec()
        })
    );
    assert!(answer.is_ok_and(|answer| answer.succeeded()));
}

#[tokio::test]
async fn a_load_carries_its_json_and_says_when_it_is_to_be_loaded_again() {
    let socket = Socket::new("admin-load");
    let held = Arc::new(Mutex::new(Held::default()));
    assert!(serve(&socket.0, Arc::clone(&held)));

    for again in [false, true] {
        let answer = Admin::at(&socket.0)
            .ask(Call {
                method: "POST",
                path: "/load",
                json: Some(br#"{"apps":{}}"#.to_vec()),
                again,
            })
            .await;
        assert!(answer.is_ok_and(|answer| answer.succeeded()));
    }

    let loads = held
        .lock()
        .map(|held| held.loads.clone())
        .unwrap_or_default();
    assert_eq!(
        loads,
        vec![(json!({"apps": {}}), false), (json!({"apps": {}}), true)]
    );
}

#[tokio::test]
async fn a_refusal_is_an_answer_that_did_not_succeed() {
    let socket = Socket::new("admin-refused");
    let held = Arc::new(Mutex::new(Held {
        refusing: Some(400),
        ..Held::default()
    }));
    assert!(serve(&socket.0, held));

    let answer = Admin::at(&socket.0).ask(get("/config/")).await;

    assert!(answer.is_ok_and(|answer| answer.status == 400 && !answer.succeeded()));
}

#[tokio::test]
async fn a_socket_nobody_listens_on_is_unreachable() {
    let socket = Socket::new("admin-nobody");

    assert_eq!(
        Admin::at(&socket.0).ask(get("/config/")).await,
        Err(Unanswered::Unreachable)
    );
}

#[tokio::test]
async fn a_path_that_is_no_uri_is_unreadable() {
    let socket = Socket::new("admin-path");
    let held = Arc::new(Mutex::new(Held::default()));
    assert!(serve(&socket.0, held));

    assert_eq!(
        Admin::at(&socket.0).ask(get("not a path")).await,
        Err(Unanswered::Unreadable)
    );
}

#[tokio::test]
async fn an_answer_larger_than_any_configuration_is_unreadable() {
    let socket = Socket::new("admin-large");
    let held = Arc::new(Mutex::new(Held {
        config: json!("x".repeat(super::MOST)),
        ..Held::default()
    }));
    assert!(serve(&socket.0, held));

    assert_eq!(
        Admin::at(&socket.0).ask(get("/config/")).await,
        Err(Unanswered::Unreadable)
    );
}

#[tokio::test]
async fn a_socket_that_never_answers_is_slow() {
    let socket = Socket::new("admin-slow");
    let Ok(listener) = std::os::unix::net::UnixListener::bind(&socket.0) else {
        return;
    };
    tokio::time::pause();

    let answer = Admin::at(&socket.0).ask(get("/config/")).await;
    drop(listener);

    assert_eq!(answer, Err(Unanswered::Slow));
}
