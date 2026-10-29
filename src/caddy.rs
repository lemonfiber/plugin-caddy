//! Caddy, told the routing table it serves by the files it reads when it starts.
//!
//! The adapter writes Caddy's Caddyfile, and the table it was written from beside it,
//! into Caddy's configuration directory, which it shares and Caddy only reads. Caddy
//! takes a table once it starts again, which the core does when told so.

pub mod config;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Proxying, Routing, Tls};

use self::config::Refused;

/// What a failure names Caddy as.
const SERVICE: &str = "Caddy";

/// The file Caddy reads its configuration from, in its configuration directory.
pub const CADDYFILE: &str = "Caddyfile";

/// The table the Caddyfile was written from, beside it.
pub const TABLE: &str = "routing.json";

/// One Caddy, told what to serve through its configuration directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caddy {
    directory: PathBuf,
}

impl Caddy {
    /// The Caddy whose configuration directory is shared at `directory`.
    #[must_use]
    pub fn at(directory: &Path) -> Self {
        Self {
            directory: directory.to_path_buf(),
        }
    }

    /// Whether Caddy's configuration directory is there to write.
    pub async fn answers(&self) -> bool {
        tokio::fs::metadata(&self.directory)
            .await
            .is_ok_and(|held| held.is_dir())
    }

    /// The table last written, or none where nothing was.
    async fn written(&self) -> Result<Option<Routing>, Failure> {
        match tokio::fs::read(self.directory.join(TABLE)).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| refused("the table written could not be read")),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(unavailable()),
        }
    }

    /// Write `contents` to `file` in Caddy's configuration directory, in one step.
    async fn put(&self, file: &str, contents: &[u8]) -> Result<(), Failure> {
        let staged = self.directory.join(format!(".{file}.writing"));
        tokio::fs::write(&staged, contents)
            .await
            .map_err(|_| unavailable())?;
        tokio::fs::rename(&staged, self.directory.join(file))
            .await
            .map_err(|_| unavailable())
    }

    /// The Caddyfile in Caddy's configuration directory.
    async fn caddyfile(&self) -> Result<String, Failure> {
        tokio::fs::read_to_string(self.directory.join(CADDYFILE))
            .await
            .map_err(|_| unavailable())
    }
}

#[async_trait]
impl Proxying for Caddy {
    async fn apply(&self, table: &Routing) -> Result<(), Failure> {
        let caddyfile = config::rendered(table).map_err(|why| refused(&said(&why)))?;
        let written =
            serde_json::to_vec(table).map_err(|_| refused("the table could not be written"))?;
        self.put(TABLE, &written).await?;
        self.put(CADDYFILE, caddyfile.as_bytes()).await?;
        if &self.routing().await? == table {
            Ok(())
        } else {
            Err(refused("the table written reads back as another"))
        }
    }

    async fn routing(&self) -> Result<Routing, Failure> {
        let Some(table) = self.written().await? else {
            return Ok(Routing {
                tls: Tls::Automatic,
                routes: Vec::new(),
            });
        };
        let rendered = config::rendered(&table).map_err(|why| refused(&said(&why)))?;
        if self.caddyfile().await? == rendered {
            Ok(table)
        } else {
            Err(refused(
                "its Caddyfile is not the one written from the table",
            ))
        }
    }

    async fn reload(&self) -> Result<(), Failure> {
        self.routing().await.map(|_| ())
    }
}

/// What a refusal of the table says.
fn said(why: &Refused) -> String {
    match why {
        Refused::Outside(file) => {
            format!("{file} is not a file beneath the proxy's configuration directory")
        }
        Refused::Unnamed(name) => format!("{name} is not a plain name"),
    }
}

/// A refusal saying `detail`.
fn refused(detail: &str) -> Failure {
    Failure::Refused {
        service: SERVICE.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Caddy's configuration directory, not there to read or write.
fn unavailable() -> Failure {
    Failure::Unavailable {
        service: SERVICE.to_owned(),
    }
}

#[cfg(test)]
mod tests;
