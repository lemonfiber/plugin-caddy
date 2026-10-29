use std::path::{Path, PathBuf};

use lemonfiber_ports::service::{Failure, Proxying, Route, Routing, Tls};

use super::{config, Caddy, CONFIGURATION, TABLE};

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
async fn a_table_applied_is_written_as_caddys_configuration_and_reads_back() {
    let directory = Directory::new("applied");
    let caddy = caddy(&directory.0);

    assert!(caddy.apply(&table()).await.is_ok());

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&directory.file(CONFIGURATION)).ok(),
        config::rendered(&table()).ok()
    );
    assert_eq!(caddy.routing().await.ok(), Some(table()));
    assert!(!directory.file(TABLE).is_empty());
}

#[tokio::test]
async fn nothing_written_yet_reads_as_no_route() {
    let directory = Directory::new("unwritten");
    let _ = std::fs::write(
        directory.0.join(CONFIGURATION),
        "{\"admin\": {\"disabled\": true}}",
    );

    assert_eq!(
        caddy(&directory.0).routing().await.ok(),
        Some(Routing {
            tls: Tls::Automatic,
            routes: Vec::new(),
        })
    );
}

#[tokio::test]
async fn a_configuration_changed_by_hand_is_refused_rather_than_read() {
    let directory = Directory::new("edited");
    let caddy = caddy(&directory.0);
    assert!(caddy.apply(&table()).await.is_ok());
    let _ = std::fs::write(
        directory.0.join(CONFIGURATION),
        "{\"admin\": {\"listen\": \"localhost:2019\"}}",
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
    let before = directory.file(CONFIGURATION);

    assert!(caddy.reload().await.is_ok());

    assert_eq!(directory.file(CONFIGURATION), before);
}

#[tokio::test]
async fn a_route_carrying_a_directive_is_refused_before_anything_is_written() {
    let directory = Directory::new("unnamed");
    let caddy = caddy(&directory.0);
    let mut unnamed = table();
    unnamed.routes = vec![Route {
        name: "a}\n:80 {\n\tfile_server".to_owned(),
        host: "door".to_owned(),
        port: 8096,
    }];

    assert!(matches!(
        caddy.apply(&unnamed).await,
        Err(Failure::Refused { .. })
    ));
    assert!(directory.file(CONFIGURATION).is_empty());
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

#[tokio::test]
async fn a_link_planted_at_the_configuration_is_replaced_and_never_written_through() {
    let directory = Directory::new("linked-configuration");
    let outside = Directory::new("linked-configuration-outside");
    let _ = std::fs::write(outside.0.join(CONFIGURATION), "theirs");
    let _ = std::os::unix::fs::symlink(
        outside.0.join(CONFIGURATION),
        directory.0.join(CONFIGURATION),
    );

    assert!(caddy(&directory.0).apply(&table()).await.is_ok());

    assert_eq!(outside.file(CONFIGURATION), "theirs");
    let held = std::fs::symlink_metadata(directory.0.join(CONFIGURATION));
    assert!(held.is_ok_and(|held| held.file_type().is_file()));
}

#[tokio::test]
async fn a_table_that_is_a_link_or_not_a_plain_file_is_refused_naming_it() {
    let directory = Directory::new("linked-table");
    let outside = Directory::new("linked-table-outside");
    let _ = std::fs::write(outside.0.join(TABLE), "{}");
    let _ = std::os::unix::fs::symlink(outside.0.join(TABLE), directory.0.join(TABLE));
    let unplain = Directory::new("unplain-table");
    let _ = std::fs::create_dir_all(unplain.0.join(TABLE));

    for at in [&directory, &unplain] {
        let refused = caddy(&at.0).routing().await;
        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains(TABLE)),
            "{refused:?}"
        );
    }
}

#[tokio::test]
async fn a_table_with_no_configuration_beside_it_is_refused() {
    let directory = Directory::new("unconfigured");
    let caddy = caddy(&directory.0);
    assert!(caddy.apply(&table()).await.is_ok());
    let _ = std::fs::remove_file(directory.0.join(CONFIGURATION));

    assert!(matches!(
        caddy.routing().await,
        Err(Failure::Refused { .. })
    ));
}
