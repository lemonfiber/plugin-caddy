//! Caddy, told the routing table it serves by the configuration it reads when it starts.
//!
//! The adapter writes Caddy's JSON configuration, and the table it was written from
//! beside it, into Caddy's configuration directory, which it shares and Caddy only
//! reads. Caddy takes a table once it starts again, which the core does when told so.

pub mod config;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Proxying, Routing, Tls};
use serde_json::Value;

use self::config::Refused;

/// What a failure names Caddy as.
const SERVICE: &str = "Caddy";

/// The file Caddy reads its configuration from, in its configuration directory.
pub const CONFIGURATION: &str = "caddy.json";

/// The table the configuration was written from, beside it.
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

    /// The configuration in Caddy's configuration directory.
    async fn configuration(&self) -> Result<Value, Failure> {
        let bytes = tokio::fs::read(self.directory.join(CONFIGURATION))
            .await
            .map_err(|_| unavailable())?;
        serde_json::from_slice(&bytes).map_err(|_| refused("its configuration could not be read"))
    }
}

#[async_trait]
impl Proxying for Caddy {
    async fn apply(&self, table: &Routing) -> Result<(), Failure> {
        let configuration = config::rendered(table).map_err(|why| refused(&said(&why)))?;
        let written =
            serde_json::to_vec(table).map_err(|_| refused("the table could not be written"))?;
        let configured = serde_json::to_vec_pretty(&configuration)
            .map_err(|_| refused("the configuration could not be written"))?;
        self.put(TABLE, &written).await?;
        self.put(CONFIGURATION, &configured).await?;
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
        if self.configuration().await? == rendered {
            Ok(table)
        } else {
            Err(refused(
                "its configuration is not the one written from the table",
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
            format!("{file:?} is not a plain file beneath the proxy's configuration directory")
        }
        Refused::Unnamed(name) => format!("{name:?} is not a plain DNS name"),
        Refused::Unhosted(host) => format!("{host:?} is not a DNS name or an IPv4 address"),
        Refused::Unported(port) => format!("{port} is not a port a service answers on"),
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
