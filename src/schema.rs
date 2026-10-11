//! The generated command tree, embedded from `src/schema.json`.
//!
//! Field order matters: `tree` and `describe -o json` print these structs as
//! they are declared.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
pub struct Schema {
    pub api: Api,
    pub resources: Vec<Resource>,
}

#[derive(Deserialize, Serialize)]
pub struct Api {
    pub title: String,
    pub version: String,
    pub base_url: String,
    pub auth_header: String,
    pub docs_url: String,
    pub spec_url: String,
}

#[derive(Deserialize, Serialize)]
pub struct Resource {
    pub name: String,
    pub description: String,
    pub ops: Vec<Op>,
}

#[derive(Deserialize, Serialize)]
pub struct Op {
    pub name: String,
    /// The API's own name, e.g. `addDomain`.
    pub operation: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub destructive: bool,
    /// A hand-written command; its params describe the command, not a request body.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub custom: bool,
    /// The one param that may be given as the first argument instead of a flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positional: Option<String>,
    pub params: Vec<Param>,
    pub returns: Vec<Field>,
}

/// How a flag's text becomes a JSON value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    String,
    Int,
    Float,
    Bool,
    Json,
    List,
}

/// One field of an operation's JSON request body.
#[derive(Deserialize, Serialize)]
pub struct Param {
    pub name: String,
    pub flag: String,
    /// The spec's own type, shown by `describe`.
    #[serde(rename = "type")]
    pub ty: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Kind>,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One field of an operation's result, as a dotted path.
#[derive(Deserialize, Serialize)]
pub struct Field {
    pub path: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub fn load() -> Schema {
    serde_json::from_str(include_str!("schema.json")).expect("src/schema.json is generated")
}

impl Schema {
    pub fn resource(&self, name: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.name == name)
    }

    pub fn resource_names(&self) -> Vec<&str> {
        self.resources.iter().map(|r| r.name.as_str()).collect()
    }

    /// Finds a generated command by the API's operation name, skipping
    /// hand-written commands that merely call it.
    pub fn by_operation(&self, operation: &str) -> Option<(&Resource, &Op)> {
        self.resources.iter().find_map(|r| {
            r.ops
                .iter()
                .find(|o| o.operation == operation && !o.custom)
                .map(|o| (r, o))
        })
    }
}

impl Resource {
    pub fn op(&self, name: &str) -> Option<&Op> {
        self.ops.iter().find(|o| o.name == name)
    }

    pub fn op_names(&self) -> Vec<&str> {
        self.ops.iter().map(|o| o.name.as_str()).collect()
    }
}

impl Op {
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.params
            .iter()
            .find(|p| p.name == name || p.flag == name)
    }
}

/// Names close to a mistyped one: prefixes and substrings first, then
/// anything within a small edit distance.
pub fn suggest<'a>(name: &str, candidates: &[&'a str], limit: usize) -> Vec<&'a str> {
    let lower = name.to_lowercase();
    let budget = lower.len() / 3 + 1;
    let (mut prefix, mut substring, mut near) = (Vec::new(), Vec::new(), Vec::new());
    for &n in candidates {
        let ln = n.to_lowercase();
        if ln.starts_with(&lower) {
            prefix.push(n);
        } else if ln.contains(&lower) || (ln.len() > 2 && lower.contains(&ln)) {
            substring.push(n);
        } else if edit_distance(&lower, &ln) <= budget {
            near.push(n);
        }
    }
    prefix
        .into_iter()
        .chain(substring)
        .chain(near)
        .take(limit)
        .collect()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}
