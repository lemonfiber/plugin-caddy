//! Caddy's admin API, asked over the unix socket it listens on and nowhere else.

use std::path::{Path, PathBuf};
use std::time::Duration;

use http_body_util::{BodyExt as _, Full, Limited};
use hyper::body::Bytes;
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;

/// How long Caddy has to answer one call, a configuration it is loading included.
const ANSWER_WITHIN: Duration = Duration::from_secs(30);

/// The most of an answer read, in bytes: a configuration with every route the stack has.
const MOST: usize = 4 * 1024 * 1024;

/// The host every call names, which Caddy's admin API takes on a unix socket.
const HOST: &str = "localhost";

/// What Caddy answered: its status and its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The HTTP status.
    pub status: u16,
    /// The body, as far as [`MOST`] bytes.
    pub body: Vec<u8>,
}

impl Answer {
    /// Whether Caddy took the call.
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

/// Why a call did not come back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unanswered {
    /// The socket could not be reached or the connection broke.
    Unreachable,
    /// Caddy did not answer within [`ANSWER_WITHIN`].
    Slow,
    /// The answer was larger than [`MOST`] bytes, or not HTTP.
    Unreadable,
}

/// One call Caddy is asked: its method, its path, the JSON it carries and whether Caddy
/// is to load a configuration it already runs again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call<'a> {
    /// The HTTP method.
    pub method: &'a str,
    /// The path on the admin API.
    pub path: &'a str,
    /// The JSON body, where it carries one.
    pub json: Option<Vec<u8>>,
    /// Whether Caddy loads the configuration even where it is the one it runs.
    pub again: bool,
}

/// Caddy's admin API at the socket `socket`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admin {
    socket: PathBuf,
}

impl Admin {
    /// The admin API listening at `socket`.
    #[must_use]
    pub fn at(socket: &Path) -> Self {
        Self {
            socket: socket.to_path_buf(),
        }
    }

    /// Ask `call`, and what Caddy answered.
    ///
    /// # Errors
    ///
    /// [`Unanswered`] where the socket cannot be reached, Caddy is slow, or its answer
    /// cannot be read.
    pub async fn ask(&self, call: Call<'_>) -> Result<Answer, Unanswered> {
        tokio::time::timeout(ANSWER_WITHIN, self.asked(call))
            .await
            .map_err(|_| Unanswered::Slow)?
    }

    async fn asked(&self, call: Call<'_>) -> Result<Answer, Unanswered> {
        let stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| Unanswered::Unreachable)?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| Unanswered::Unreachable)?;
        tokio::spawn(connection);
        let mut request = Request::builder()
            .method(call.method)
            .uri(call.path)
            .header(hyper::header::HOST, HOST);
        if call.json.is_some() {
            request = request.header(hyper::header::CONTENT_TYPE, "application/json");
        }
        if call.again {
            request = request.header(hyper::header::CACHE_CONTROL, "must-revalidate");
        }
        let request = request
            .body(Full::new(Bytes::from(call.json.unwrap_or_default())))
            .map_err(|_| Unanswered::Unreadable)?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| Unanswered::Unreachable)?;
        let status = response.status().as_u16();
        let body = Limited::new(response.into_body(), MOST)
            .collect()
            .await
            .map_err(|_| Unanswered::Unreadable)?
            .to_bytes()
            .to_vec();
        Ok(Answer { status, body })
    }
}

#[cfg(test)]
mod tests;
