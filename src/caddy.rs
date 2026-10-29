//! Caddy, told the routing table it serves by the configuration it reads when it starts.
//!
//! The adapter writes Caddy's JSON configuration, and the table it was written from
//! beside it, into Caddy's configuration directory, which it shares and Caddy only
//! reads. Caddy takes a table once it starts again, which the core does when told so.

pub mod config;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lemonfiber_adapter_kit::shared::{Shared, SharedName, Unshared};
use lemonfiber_ports::service::{Failure, Proxying, Routing, Tls};
use serde_json::Value;

use self::config::Refused;

/// What a failure names Caddy as.
const SERVICE: &str = "Caddy";

/// The file Caddy reads its configuration from, in its configuration directory.
pub const CONFIGURATION: &str = "caddy.json";

/// The mode every file is written with: Caddy reads it, and only the adapter writes it.
const MODE: u32 = 0o644;

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

    /// What `file` holds, or none where it is not there.
    async fn read(&self, file: &str) -> Result<Option<Vec<u8>>, Failure> {
        Shared::at(&self.directory)
            .read(&SharedName::new(file).map_err(unshared)?)
            .await
            .map_err(unshared)
    }

    /// Write `contents` to `file` in Caddy's configuration directory, in one step.
    async fn put(&self, file: &str, contents: Vec<u8>) -> Result<(), Failure> {
        Shared::at(&self.directory)
            .write(&SharedName::new(file).map_err(unshared)?, contents, MODE)
            .await
            .map_err(unshared)
    }

    /// The table last written, or none where nothing was.
    async fn written(&self) -> Result<Option<Routing>, Failure> {
        self.read(TABLE)
            .await?
            .map(|bytes| {
                serde_json::from_slice(&bytes)
                    .map_err(|_| refused("the table written could not be read"))
            })
            .transpose()
    }

    /// The configuration in Caddy's configuration directory.
    async fn configuration(&self) -> Result<Value, Failure> {
        let bytes = self
            .read(CONFIGURATION)
            .await?
            .ok_or_else(|| refused("its configuration is not there"))?;
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
        self.put(TABLE, written).await?;
        self.put(CONFIGURATION, configured).await?;
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
        Refused::Twice(name) => format!("{name:?} is the name of two routes"),
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

/// A file in Caddy's configuration directory that could not be reached: a link or a
/// file that is not plain is refused by name, and a directory that is not there to read
/// or write leaves Caddy unavailable.
fn unshared(why: Unshared) -> Failure {
    match why {
        Unshared::Failed(..) => Failure::Unavailable {
            service: SERVICE.to_owned(),
        },
        refusal => refused(&refusal.to_string()),
    }
}

#[cfg(test)]
mod tests;
