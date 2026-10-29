//! lemonfiber's adapter for Caddy: the `proxy.front` contract, spoken beside one Caddy and
//! applied by writing the JSON configuration Caddy reads when it starts, in Caddy's configuration
//! directory, which the adapter shares and Caddy only reads.

pub mod adapter;
pub mod caddy;
