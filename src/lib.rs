//! lemonfiber's adapter for Caddy: the `proxy.front` contract, spoken beside one Caddy and
//! applied through its admin API, which it reaches over a socket in Caddy's configuration
//! directory alone.

pub mod adapter;
pub mod caddy;

#[cfg(test)]
mod fake;
