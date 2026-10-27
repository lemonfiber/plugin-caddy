//! A routing table as the configuration Caddy loads, and the table a configuration Caddy
//! runs serves.
//!
//! Every name is one route of one server, matched on its host and handed to the service
//! behind it, so the table reads back from the configuration exactly as it was applied.

use lemonfiber_ports::service::{Route, Routing, Tls};
use serde_json::{json, Value};

/// Where Caddy's configuration directory is mounted inside its own container.
pub const CADDY_CONFIGURATION: &str = "/etc/caddy";

/// Where Caddy's admin API listens, inside its own container: a socket in its
/// configuration directory, readable and writable by its own user alone.
pub const ADMIN_LISTEN: &str = "unix//etc/caddy/admin.sock|0600";

/// The port Caddy answers plain HTTP on inside its container.
pub const HTTP_PORT: u16 = 8080;

/// The port Caddy answers HTTPS on inside its container.
pub const HTTPS_PORT: u16 = 8443;

/// The one server every route is a route of.
const SERVER: &str = "lemonfiber";

/// Why a table or a configuration was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A certificate or key named a file outside the proxy's configuration directory.
    Outside(String),
    /// A name or a host that is not one word of letters, digits, dots and hyphens.
    Unnamed(String),
    /// The configuration Caddy runs is not one a table was applied as.
    Unread(&'static str),
}

/// The configuration Caddy loads to serve `table`.
///
/// # Errors
///
/// [`Refused`] where a certificate's file lies outside the proxy's configuration
/// directory, or a name or host is not a plain DNS name.
pub fn loaded(table: &Routing) -> Result<Value, Refused> {
    let routes = table
        .routes
        .iter()
        .map(route)
        .collect::<Result<Vec<_>, _>>()?;
    let mut server = json!({
        "listen": [format!(":{HTTP_PORT}"), format!(":{HTTPS_PORT}")],
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
        "admin": { "listen": ADMIN_LISTEN },
        "apps": apps,
    }))
}

/// One route of the table, as a route of the server.
fn route(route: &Route) -> Result<Value, Refused> {
    Ok(json!({
        "match": [{ "host": [named(&route.name)?] }],
        "handle": [{
            "handler": "reverse_proxy",
            "upstreams": [{ "dial": format!("{}:{}", named(&route.host)?, route.port) }],
        }],
        "terminal": true,
    }))
}

/// `name`, where it is a plain DNS name.
fn named(name: &str) -> Result<&str, Refused> {
    let plain = !name.is_empty()
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-');
    if plain {
        Ok(name)
    } else {
        Err(Refused::Unnamed(name.to_owned()))
    }
}

/// `file` beneath the proxy's configuration directory, as Caddy names it.
fn beneath(file: &str) -> Result<String, Refused> {
    let plain = !file.is_empty()
        && file
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if plain {
        Ok(format!("{CADDY_CONFIGURATION}/{file}"))
    } else {
        Err(Refused::Outside(file.to_owned()))
    }
}

/// The table `config`, a configuration Caddy runs, serves: none where Caddy runs no
/// server of a table's.
///
/// # Errors
///
/// [`Refused::Unread`] where the configuration holds the server in a shape no table is
/// loaded as.
pub fn served(config: &Value) -> Result<Routing, Refused> {
    let Some(server) = config.pointer(&format!("/apps/http/servers/{SERVER}")) else {
        return Ok(Routing {
            tls: Tls::Automatic,
            routes: Vec::new(),
        });
    };
    let routes = server.get("routes").and_then(Value::as_array).map_or_else(
        || Ok(Vec::new()),
        |routes| routes.iter().map(read_route).collect(),
    )?;
    Ok(Routing {
        tls: tls(config)?,
        routes,
    })
}

/// The TLS `config` serves its names with.
fn tls(config: &Value) -> Result<Tls, Refused> {
    let Some(files) = config.pointer("/apps/tls/certificates/load_files/0") else {
        return Ok(Tls::Automatic);
    };
    let file = |field: &str| {
        files
            .get(field)
            .and_then(Value::as_str)
            .and_then(|path| path.strip_prefix(&format!("{CADDY_CONFIGURATION}/")))
            .map(str::to_owned)
            .ok_or(Refused::Unread(
                "a certificate's files are not the proxy's own",
            ))
    };
    Ok(Tls::Files {
        certificate: file("certificate")?,
        key: file("key")?,
    })
}

/// One route as the server holds it.
fn read_route(route: &Value) -> Result<Route, Refused> {
    let name = route
        .pointer("/match/0/host/0")
        .and_then(Value::as_str)
        .ok_or(Refused::Unread("a route matches no one name"))?;
    let dial = route
        .pointer("/handle/0/upstreams/0/dial")
        .and_then(Value::as_str)
        .ok_or(Refused::Unread("a route hands nothing on"))?;
    let (host, port) = dial
        .rsplit_once(':')
        .and_then(|(host, port)| Some((host, port.parse().ok()?)))
        .ok_or(Refused::Unread("a route hands on to no host and port"))?;
    Ok(Route {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
    })
}

#[cfg(test)]
mod tests;
