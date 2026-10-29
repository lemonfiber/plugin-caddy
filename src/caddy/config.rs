//! A routing table as the Caddyfile Caddy reads when it starts.
//!
//! Every name is one site of the household's, handed to the service behind it, under
//! a global block that keeps Caddy's admin API off: nothing reaches Caddy's running
//! configuration, and what it serves is what this file says once it starts.

use lemonfiber_ports::service::{Route, Routing, Tls};

/// Where Caddy's configuration directory is mounted inside its own container.
pub const CADDY_CONFIGURATION: &str = "/etc/caddy";

/// The port Caddy answers plain HTTP on inside its container.
pub const HTTP_PORT: u16 = 8080;

/// The port Caddy answers HTTPS on inside its container.
pub const HTTPS_PORT: u16 = 8443;

/// The first line of every Caddyfile this adapter writes.
pub const WRITTEN: &str = "# Written by caddy-adapter from lemonfiber's routing table.";

/// Why a table was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A certificate or key named a file outside the proxy's configuration directory.
    Outside(String),
    /// A name or a host that is not one word of letters, digits, dots and hyphens.
    Unnamed(String),
}

/// The Caddyfile that serves `table`.
///
/// # Errors
///
/// [`Refused`] where a certificate's file lies outside the proxy's configuration
/// directory, or a name or host is not a plain DNS name.
pub fn rendered(table: &Routing) -> Result<String, Refused> {
    let tls = match &table.tls {
        Tls::Automatic => None,
        Tls::Files { certificate, key } => Some(format!(
            "\ttls {} {}\n",
            beneath(certificate)?,
            beneath(key)?
        )),
    };
    let mut text = format!(
        "{WRITTEN}\n\n{{\n\tadmin off\n\tpersist_config off\n\thttp_port {HTTP_PORT}\n\thttps_port {HTTPS_PORT}\n}}\n"
    );
    for route in &table.routes {
        text.push_str(&site(route, tls.as_deref())?);
    }
    Ok(text)
}

/// One route of the table, as a site of the Caddyfile.
fn site(route: &Route, tls: Option<&str>) -> Result<String, Refused> {
    Ok(format!(
        "\n{} {{\n{}\treverse_proxy {}:{}\n}}\n",
        named(&route.name)?,
        tls.unwrap_or_default(),
        named(&route.host)?,
        route.port
    ))
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

#[cfg(test)]
mod tests;
