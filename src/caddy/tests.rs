use std::path::{Path, PathBuf};

use lemonfiber_ports::service::{Failure, Proxying, Route, Routing, Tls};

use super::{config, Caddy, CADDYFILE, TABLE};

/// A configuration directory of its own, removed when dropped.
struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("caddy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }

    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap_or_default()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

fn caddy(directory: &Path) -> Caddy {
    Caddy::at(directory)
}

#[tokio::test]
async fn a_table_applied_is_written_as_the_caddyfile_and_reads_back() {
    let directory = Directory::new("applied");
    let caddy = caddy(&directory.0);

    assert!(caddy.apply(&table()).await.is_ok());

    assert_eq!(
        directory.file(CADDYFILE),
        config::rendered(&table()).unwrap_or_default()
    );
    assert_eq!(caddy.routing().await.ok(), Some(table()));
    assert!(!directory.file(TABLE).is_empty());
}

#[tokio::test]
async fn nothing_written_yet_reads_as_no_route() {
    let directory = Directory::new("unwritten");
    let _ = std::fs::write(directory.0.join(CADDYFILE), "{\n\tadmin off\n}\n");

    assert_eq!(
        caddy(&directory.0).routing().await.ok(),
        Some(Routing {
            tls: Tls::Automatic,
            routes: Vec::new(),
        })
    );
}

#[tokio::test]
async fn a_caddyfile_changed_by_hand_is_refused_rather_than_read() {
    let directory = Directory::new("edited");
    let caddy = caddy(&directory.0);
    assert!(caddy.apply(&table()).await.is_ok());
    let _ = std::fs::write(
        directory.0.join(CADDYFILE),
        "{\n\tadmin localhost:2019\n}\n",
    );

    assert!(matches!(
        caddy.routing().await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(caddy.reload().await, Err(Failure::Refused { .. })));
}

#[tokio::test]
async fn a_reload_reads_the_written_table_and_writes_nothing() {
    let directory = Directory::new("reloaded");
    let caddy = caddy(&directory.0);
    assert!(caddy.apply(&table()).await.is_ok());
    let before = directory.file(CADDYFILE);

    assert!(caddy.reload().await.is_ok());

    assert_eq!(directory.file(CADDYFILE), before);
}

#[tokio::test]
async fn a_name_that_is_not_plain_is_refused_before_anything_is_written() {
    let directory = Directory::new("unnamed");
    let caddy = caddy(&directory.0);
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
    assert!(directory.file(CADDYFILE).is_empty());
    assert!(directory.file(TABLE).is_empty());
}

#[tokio::test]
async fn a_directory_that_is_not_there_is_unavailable() {
    let caddy = caddy(Path::new("/nonexistent/caddy-configuration"));

    assert!(!caddy.answers().await);
    assert!(matches!(
        caddy.apply(&table()).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_table_that_cannot_be_read_is_refused() {
    let directory = Directory::new("unreadable");
    let _ = std::fs::write(directory.0.join(TABLE), "not json");

    assert!(matches!(
        caddy(&directory.0).routing().await,
        Err(Failure::Refused { .. })
    ));
}
