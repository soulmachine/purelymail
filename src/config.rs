//! Token resolution: `--token`, then `PURELYMAIL_API_TOKEN`, then the selected
//! profile in the config file.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use serde::Deserialize;

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

/// ```toml
/// api_token = "pm-live-..."
///
/// [client-b]
/// api_token = "pm-live-..."
/// ```
#[derive(Deserialize, Default)]
struct File {
    api_token: Option<String>,
    #[serde(flatten)]
    profiles: BTreeMap<String, Profile>,
}

#[derive(Deserialize, Default)]
struct Profile {
    api_token: Option<String>,
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
    let file = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<File>(&text)
            .map_err(|e| Error::Usage(format!("{}: {e}", path.display())))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => File::default(),
        Err(e) => return Err(Error::Usage(format!("open {}: {e}", path.display()))),
    };

    let profile = profile
        .map(str::to_owned)
        .or_else(|| nonempty(env::var(ENV_PROFILE)))
        .unwrap_or_else(|| DEFAULT_PROFILE.to_owned());
    let from_file = if profile == DEFAULT_PROFILE {
        file.api_token
            .or_else(|| file.profiles.get(DEFAULT_PROFILE)?.api_token.clone())
    } else {
        file.profiles
            .get(&profile)
            .ok_or_else(|| {
                Error::Usage(format!(
                    "profile {profile:?} not found in {}",
                    path.display()
                ))
            })?
            .api_token
            .clone()
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

fn nonempty(v: Result<String, env::VarError>) -> Option<String> {
    v.ok().filter(|s| !s.is_empty())
}
