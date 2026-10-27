//! Caddy, told the routing table it serves through its admin API.

pub mod admin;
pub mod config;

use std::path::Path;

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Proxying, Routing};
use serde_json::Value;

use self::admin::{Admin, Answer, Call, Unanswered};
use self::config::Refused;

/// What a failure names Caddy as.
const SERVICE: &str = "Caddy";

/// Where the configuration Caddy runs is read and a new one loaded.
const CONFIG: &str = "/config/";

/// Where Caddy loads a configuration in place of the one it runs.
const LOAD: &str = "/load";

/// One Caddy, asked through the admin API at its socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caddy {
    admin: Admin,
}

impl Caddy {
    /// The Caddy whose admin API listens at `socket`.
    #[must_use]
    pub fn at(socket: &Path) -> Self {
        Self {
            admin: Admin::at(socket),
        }
    }

    /// Whether Caddy answers its admin API.
    pub async fn answers(&self) -> bool {
        self.running().await.is_ok()
    }

    /// The configuration Caddy runs.
    async fn running(&self) -> Result<Value, Failure> {
        let answer = self
            .asked(Call {
                method: "GET",
                path: CONFIG,
                json: None,
                again: false,
            })
            .await?;
        serde_json::from_slice(&answer.body)
            .map_err(|_| refused("its configuration could not be read"))
    }

    /// Load `config` in place of the configuration Caddy runs, `again` where it is the
    /// same one.
    async fn load(&self, config: &Value, again: bool) -> Result<(), Failure> {
        self.asked(Call {
            method: "POST",
            path: LOAD,
            json: Some(config.to_string().into_bytes()),
            again,
        })
        .await
        .map(|_| ())
    }

    /// Ask `call`, and Caddy's answer where it took it.
    async fn asked(&self, call: Call<'_>) -> Result<Answer, Failure> {
        let answer = self.admin.ask(call).await.map_err(|why| match why {
            Unanswered::Unreachable | Unanswered::Slow => Failure::Unavailable {
                service: SERVICE.to_owned(),
            },
            Unanswered::Unreadable => refused("its answer could not be read"),
        })?;
        if answer.succeeded() {
            Ok(answer)
        } else {
            Err(refused(&format!("it answered {}", answer.status)))
        }
    }
}

#[async_trait]
impl Proxying for Caddy {
    async fn apply(&self, table: &Routing) -> Result<(), Failure> {
        let config = config::loaded(table).map_err(|why| refused(&said(&why)))?;
        self.load(&config, false).await?;
        let served = self.routing().await?;
        if &served == table {
            Ok(())
        } else {
            Err(refused("the table loaded reads back as another"))
        }
    }

    async fn routing(&self) -> Result<Routing, Failure> {
        config::served(&self.running().await?).map_err(|why| refused(&said(&why)))
    }

    async fn reload(&self) -> Result<(), Failure> {
        let running = self.running().await?;
        self.load(&running, true).await
    }
}

/// What a refusal of the table says.
fn said(why: &Refused) -> String {
    match why {
        Refused::Outside(file) => {
            format!("{file} is not a file beneath the proxy's configuration directory")
        }
        Refused::Unnamed(name) => format!("{name} is not a plain name"),
        Refused::Unread(what) => (*what).to_owned(),
    }
}

/// A refusal saying `detail`.
fn refused(detail: &str) -> Failure {
    Failure::Refused {
        service: SERVICE.to_owned(),
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests;
