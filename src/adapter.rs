//! What this adapter says it is, and the kit that serves its contract.

use std::sync::Arc;

use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::proxy;

use crate::caddy::Caddy;

/// Where Caddy's configuration directory is, in the adapter's container, which the
/// adapter shares and Caddy only reads.
pub const ADDRESS: &str = "/caddy";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Caddy";

/// The Caddy release this adapter's recordings were taken from.
pub const RELEASE: &str = "v2.11.7";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:46c69e82a029f503a44fe9d40467e6ba3c59fd8a80e049550fa4655ccb63ce16";

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            proxy::front::CAPABILITY,
            proxy::front::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The contract, served from `caddy` under `key`, ready while Caddy's configuration
/// directory is there to write.
#[must_use]
pub fn kit(key: String, caddy: Arc<Caddy>) -> Kit {
    let asked = Arc::clone(&caddy);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(proxy::front::served(caddy))
}

#[cfg(test)]
mod tests;
