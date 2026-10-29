//! A routing table as the JSON configuration Caddy reads when it starts, and every value
//! of the table held to a strict grammar before anything is written.
//!
//! Every value in the table comes from a `proxy.front@1` request, so from any plugin's
//! routes. Each is refused unless it is a plain name, an address or a file name of the
//! shapes below, and a table is refused where two routes answer on one name or a route
//! answers on a wildcard, so no route answers for another's name, before anything is
//! written, and what is written is JSON that serde
//! quotes, so no value can carry a directive, a placeholder or a block of its own. Caddy's
//! admin API is off in every configuration written: nothing reaches its running
//! configuration, and what it serves is what this file says once it starts.

use std::net::Ipv4Addr;

use lemonfiber_ports::service::{Route, Routing, Tls};
use serde_json::{json, Value};

/// Where Caddy's configuration directory is mounted inside its own container.
pub const CADDY_CONFIGURATION: &str = "/etc/caddy";

/// The port Caddy answers plain HTTP on inside its container, which it redirects.
pub const HTTP_PORT: u16 = 8080;

/// The port Caddy answers HTTPS on inside its container.
pub const HTTPS_PORT: u16 = 8443;

/// The files beneath the configuration directory that are the adapter's own, which a
/// certificate may never be read from.
pub const OWN: [&str; 2] = ["caddy.json", "routing.json"];

/// The one server every route is a route of.
const SERVER: &str = "lemonfiber";

/// The longest name DNS carries.
const LONGEST_NAME: usize = 253;

/// The longest label DNS carries.
const LONGEST_LABEL: usize = 63;

/// The most segments a certificate's file is named by.
const DEEPEST_FILE: usize = 4;

/// The longest segment of a certificate's file name.
const LONGEST_SEGMENT: usize = 64;

/// Why a table was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A certificate or key that is not a plain file beneath the configuration directory.
    Outside(String),
    /// A name that is not a DNS name of letters, digits and hyphens. A wildcard is one
    /// too, since it would answer for names the table gives other routes.
    Unnamed(String),
    /// A name two routes of the table answer on, which one of them would take from the
    /// other.
    Twice(String),
    /// A host that is not a DNS name or an IPv4 address.
    Unhosted(String),
    /// A port that no service answers on.
    Unported(u16),
}

/// The configuration Caddy loads to serve `table`, every value of it held to its
/// grammar first.
///
/// # Errors
///
/// [`Refused`] naming the first value that is not of its grammar.
pub fn rendered(table: &Routing) -> Result<Value, Refused> {
    let mut seen = std::collections::BTreeSet::new();
    for route in &table.routes {
        if !seen.insert(route.name.to_ascii_lowercase()) {
            return Err(Refused::Twice(route.name.clone()));
        }
    }
    let routes = table
        .routes
        .iter()
        .map(route)
        .collect::<Result<Vec<_>, _>>()?;
    let mut server = json!({
        "listen": [format!(":{HTTPS_PORT}")],
        "routes": routes,
    });
    let mut apps = serde_json::Map::new();
    if let Tls::Files { certificate, key } = &table.tls {
        let files = json!([{
            "certificate": beneath(certificate)?,
            "key": beneath(key)?,
        }]);
        apps.insert(
            "tls".to_owned(),
            json!({ "certificates": { "load_files": files } }),
        );
        if let Some(fields) = server.as_object_mut() {
            fields.insert(
                "automatic_https".to_owned(),
                json!({ "disable_certificates": true }),
            );
        }
    }
    apps.insert(
        "http".to_owned(),
        json!({
            "http_port": HTTP_PORT,
            "https_port": HTTPS_PORT,
            "servers": { SERVER: server },
        }),
    );
    Ok(json!({
        "admin": { "disabled": true },
        "apps": apps,
    }))
}

/// One route of the table, as a route of the server.
fn route(route: &Route) -> Result<Value, Refused> {
    if route.port == 0 {
        return Err(Refused::Unported(route.port));
    }
    let name = named(&route.name).ok_or_else(|| Refused::Unnamed(route.name.clone()))?;
    let host = hosted(&route.host).ok_or_else(|| Refused::Unhosted(route.host.clone()))?;
    Ok(json!({
        "match": [{ "host": [name] }],
        "handle": [{
            "handler": "reverse_proxy",
            "upstreams": [{ "dial": format!("{host}:{}", route.port) }],
        }],
        "terminal": true,
    }))
}

/// `name`, where it is a DNS name of letters, digits and hyphens.
fn named(name: &str) -> Option<&str> {
    dns(name).then_some(name)
}

/// `host`, where it is a DNS name of letters, digits and hyphens, or an IPv4 address.
fn hosted(host: &str) -> Option<&str> {
    (dns(host) || host.parse::<Ipv4Addr>().is_ok()).then_some(host)
}

/// Whether `name` is a DNS name: labels of letters, digits and hyphens, each 1 to 63
/// long and neither beginning nor ending with a hyphen, the last not all digits, at most
/// 253 in all.
fn dns(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= LONGEST_NAME
        && !name
            .rsplit('.')
            .next()
            .is_some_and(|last| last.bytes().all(|byte| byte.is_ascii_digit()))
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= LONGEST_LABEL
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

/// `file` beneath the proxy's configuration directory, as Caddy names it: segments of
/// letters, digits, `.`, `_` and `-`, none beginning with `.`, and never one of the
/// adapter's own files.
fn beneath(file: &str) -> Result<String, Refused> {
    let segments: Vec<&str> = file.split('/').collect();
    let plain = !file.is_empty()
        && segments.len() <= DEEPEST_FILE
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.len() <= LONGEST_SEGMENT
                && !segment.starts_with('.')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        && !OWN.contains(&file);
    if plain {
        Ok(format!("{CADDY_CONFIGURATION}/{file}"))
    } else {
        Err(Refused::Outside(file.to_owned()))
    }
}

#[cfg(test)]
mod tests;
