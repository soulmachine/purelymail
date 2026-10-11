//! The HTTP layer. Every operation is `POST <base>/api/v0/<operation>` with a
//! JSON body and the token in a header. Purelymail answers HTTP 200 for both
//! outcomes and puts the verdict in the body:
//!
//! ```json
//! {"type":"success","result":{...}}
//! {"type":"error","code":"invalidToken","message":"Token not valid."}
//! ```

use std::time::Duration;

use serde_json::{Map, Value, json};

pub const AUTH_HEADER: &str = "Purelymail-Api-Token";

/// Every failure, with the exit code it maps to.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Usage error, bad local config, or a delete refused for lack of `--yes`.
    #[error("{0}")]
    Usage(String),
    #[error(
        "no API token: set {}, pass --token, or add api_token to {}",
        crate::config::ENV_TOKEN,
        crate::config::path().display()
    )]
    NoToken,
    /// Purelymail's own `{"type":"error"}` envelope.
    #[error("{code}: {message}")]
    Api { code: String, message: String },
    #[error("http 429: rate limited")]
    RateLimited,
    /// Could not reach the API, a non-200 status, or a body that is not an envelope.
    #[error("{0}")]
    Transport(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::Usage(_) | Error::NoToken | Error::Io(_) => 1,
            Error::Api { code, .. } if code == "invalidToken" => 2,
            Error::Api { .. } => 3,
            Error::RateLimited => 4,
            Error::Transport(_) => 5,
        }
    }
}

pub struct Client {
    agent: ureq::Agent,
    base_url: String,
    token: Option<String>,
    max_retries: u32,
    pub dry_run: bool,
    verbose: bool,
}

impl Client {
    pub fn new(
        base_url: &str,
        token: Option<String>,
        max_retries: u32,
        dry_run: bool,
        verbose: bool,
    ) -> Client {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            // A redirect would carry the token to wherever it points.
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .new_agent();
        Client {
            agent,
            base_url: base_url.trim_end_matches('/').to_owned(),
            token,
            max_retries,
            dry_run,
            verbose,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Calls an operation and returns its unwrapped `result`. In dry-run mode
    /// nothing is sent and the returned document describes the request.
    pub fn call(&self, operation: &str, body: &Map<String, Value>) -> Result<Value, Error> {
        let url = format!("{}/api/v0/{operation}", self.base_url);
        if self.dry_run {
            return Ok(json!({
                "dry_run": true,
                "method": "POST",
                "url": url,
                "headers": {"Content-Type": "application/json", AUTH_HEADER: redact(self.token.as_deref())},
                "body": body,
            }));
        }
        let token = self.token.as_deref().ok_or(Error::NoToken)?;
        let payload = serde_json::to_vec(body).map_err(std::io::Error::other)?;
        // A read that is re-sent costs nothing. A create that timed out after
        // the server acted on it must not run twice.
        let retries = if ["list", "get", "check"]
            .iter()
            .any(|p| operation.starts_with(p))
        {
            self.max_retries
        } else {
            0
        };
        let mut attempt = 0;
        loop {
            self.log(&format!("POST {url}"));
            let sent = self
                .agent
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .header(AUTH_HEADER, token)
                .send(&payload[..]);
            let (status, retry_after, text) = match sent {
                Ok(mut resp) => {
                    let retry_after = resp
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok()?.trim().parse::<u64>().ok());
                    let text = resp.body_mut().read_to_string().unwrap_or_default();
                    (resp.status().as_u16(), retry_after, text)
                }
                Err(e) if attempt < retries => {
                    self.wait(attempt, None, &format!("request failed: {e}"));
                    attempt += 1;
                    continue;
                }
                Err(e) => return Err(Error::Transport(format!("request failed: {e}"))),
            };
            if matches!(status, 429 | 502 | 503 | 504) && attempt < retries {
                let server = if status == 429 { retry_after } else { None };
                self.wait(attempt, server, &format!("http {status}"));
                attempt += 1;
                continue;
            }
            return decode(status, &text);
        }
    }

    fn wait(&self, attempt: u32, retry_after: Option<u64>, why: &str) {
        let wait = match retry_after {
            Some(secs) => Duration::from_secs(secs.min(30)),
            None => Duration::from_millis(250 << attempt.min(6)),
        };
        self.log(&format!(
            "{why}; retry {} of {} in {wait:?}",
            attempt + 1,
            self.max_retries
        ));
        std::thread::sleep(wait);
    }

    fn log(&self, line: &str) {
        if self.verbose {
            eprintln!("purelymail: {line}");
        }
    }
}

fn decode(status: u16, text: &str) -> Result<Value, Error> {
    if status == 429 {
        return Err(Error::RateLimited);
    }
    if status != 200 {
        return Err(Error::Transport(format!(
            "http {status}: {}",
            first_line(text)
        )));
    }
    let not_envelope = || {
        Error::Transport(format!(
            "http 200 but not a Purelymail API envelope: {}",
            first_line(text)
        ))
    };
    let Ok(Value::Object(mut envelope)) = serde_json::from_str::<Value>(text) else {
        return Err(not_envelope());
    };
    match envelope.get("type").and_then(Value::as_str) {
        Some("success") => Ok(match envelope.remove("result") {
            None | Some(Value::Null) => json!({}),
            Some(result) => result,
        }),
        Some("error") => Err(Error::Api {
            code: string_field(&envelope, "code"),
            message: string_field(&envelope, "message"),
        }),
        _ => Err(not_envelope()),
    }
}

fn string_field(m: &Map<String, Value>, key: &str) -> String {
    m.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn first_line(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return "(empty body)".to_owned();
    }
    line.chars().take(200).collect()
}

pub fn redact(token: Option<&str>) -> String {
    match token {
        None | Some("") => "<none>".to_owned(),
        Some(t) => format!("<redacted, {} chars>", t.chars().count()),
    }
}
