//! Token resolution: `--token`, then `PURELYMAIL_API_TOKEN`, then the selected
//! profile in the config file.

use std::env;
use std::path::PathBuf;

use toml::{Table, Value};

use crate::api::Error;

pub const ENV_TOKEN: &str = "PURELYMAIL_API_TOKEN";
const ENV_PROFILE: &str = "PURELYMAIL_PROFILE";
const ENV_CONFIG: &str = "PURELYMAIL_CONFIG";
const DEFAULT_PROFILE: &str = "default";

/// The resolved credential for one invocation.
pub struct Config {
    pub profile: String,
    pub token: Option<String>,
    /// Where the token came from, for `config show`. Empty when there is none.
    pub source: String,
}

pub fn path() -> PathBuf {
    if let Some(p) = nonempty(env::var(ENV_CONFIG)) {
        return PathBuf::from(p);
    }
    let base = nonempty(env::var("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| nonempty(env::var("HOME")).map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("purelymail").join("config.toml")
}

pub fn resolve(profile: Option<&str>, token: Option<&str>) -> Result<Config, Error> {
    let path = path();
    let table = match std::fs::read_to_string(&path) {
        Ok(text) => {
            parse(&text).map_err(|why| Error::Usage(format!("{}: {why}", path.display())))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Table::new(),
        Err(e) => return Err(Error::Usage(format!("open {}: {e}", path.display()))),
    };

    let profile = profile
        .map(str::to_owned)
        .or_else(|| nonempty(env::var(ENV_PROFILE)))
        .unwrap_or_else(|| DEFAULT_PROFILE.to_owned());
    let section = table.get(&profile).and_then(Value::as_table);
    let from_file = if profile == DEFAULT_PROFILE {
        token_in(&table).or_else(|| section.and_then(token_in))
    } else {
        token_in(section.ok_or_else(|| {
            Error::Usage(format!(
                "profile {profile:?} not found in {}",
                path.display()
            ))
        })?)
    };

    let (token, source) = if let Some(t) = token.filter(|t| !t.is_empty()) {
        (Some(t.to_owned()), "--token".to_owned())
    } else if let Some(t) = nonempty(env::var(ENV_TOKEN)) {
        (Some(t), ENV_TOKEN.to_owned())
    } else if let Some(t) = from_file {
        (Some(t), format!("{} [{profile}]", path.display()))
    } else {
        (None, String::new())
    };
    Ok(Config {
        profile,
        token,
        source,
    })
}

/// The file holds tokens, so no error text may quote its content: only a line
/// number or a key name.
///
/// ```toml
/// api_token = "pm-live-..."
///
/// [client-b]
/// api_token = "pm-live-..."
/// ```
fn parse(text: &str) -> Result<Table, String> {
    let table: Table = text.parse().map_err(|e: toml::de::Error| match e.span() {
        Some(s) => format!(
            "invalid TOML at line {}",
            text.get(..s.start).unwrap_or("").matches('\n').count() + 1
        ),
        None => "invalid TOML".to_owned(),
    })?;
    for (key, value) in &table {
        let known = match value {
            Value::Table(profile) => profile.keys().all(|k| k == "api_token"),
            _ => key == "api_token",
        };
        if !known {
            return Err(format!(
                "unknown key under {key:?}; only api_token and [profile] tables with api_token are allowed"
            ));
        }
    }
    Ok(table)
}

fn token_in(section: &Table) -> Option<String> {
    section
        .get("api_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn nonempty(v: Result<String, env::VarError>) -> Option<String> {
    v.ok().filter(|s| !s.is_empty())
}
