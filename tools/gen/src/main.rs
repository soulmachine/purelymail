//! Generates `src/schema.json` from Purelymail's OpenAPI spec.
//!
//! The spec is proprietary and never committed. What is extracted is a factual
//! description of the API: operation names, paths, body fields, their types and
//! one-line descriptions. Every operation in the spec must have a row in `OPS`;
//! an unmapped one fails the run so a new endpoint cannot vanish silently.

use std::fmt;
use std::path::Path;
use std::process::ExitCode;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Number;

const SPEC_URL: &str = "https://news.purelymail.com/api/swagger-spec.js";
const DOCS_URL: &str = "https://news.purelymail.com/api/index.html";
const BASE_URL: &str = "https://purelymail.com";

/// Resources, in the order `purelymail list` shows them.
const RESOURCES: &[(&str, &str)] = &[
    ("user", "Users on your account"),
    (
        "password-reset",
        "A user's password reset methods (email or phone)",
    ),
    ("app-password", "App passwords for IMAP/SMTP clients"),
    (
        "routing",
        "Mail routing rules: aliases, forwards, catch-alls",
    ),
    ("domain", "Domains: add, list, settings, ownership proof"),
    ("account", "Account billing and credit"),
];

/// API operation, resource, command name.
const OPS: &[(&str, &str, &str)] = &[
    ("createUser", "user", "create"),
    ("getUser", "user", "get"),
    ("listUser", "user", "list"),
    ("modifyUser", "user", "modify"),
    ("deleteUser", "user", "delete"),
    ("upsertPasswordReset", "password-reset", "upsert"),
    ("listPasswordReset", "password-reset", "list"),
    ("deletePasswordReset", "password-reset", "delete"),
    ("createAppPassword", "app-password", "create"),
    ("deleteAppPassword", "app-password", "delete"),
    ("createRoutingRule", "routing", "create"),
    ("listRoutingRules", "routing", "list"),
    ("deleteRoutingRule", "routing", "delete"),
    ("addDomain", "domain", "add"),
    ("listDomains", "domain", "list"),
    ("getOwnershipCode", "domain", "ownership-code"),
    ("updateDomainSettings", "domain", "update"),
    ("deleteDomain", "domain", "delete"),
    ("checkAccountCredit", "account", "credit"),
];

/// JSON with object keys kept in document order, which `serde_json::Value`
/// drops and the output's field order depends on.
#[derive(Clone, Debug, PartialEq)]
enum J {
    Null,
    Bool(bool),
    Num(Number),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

static EMPTY: J = J::Obj(Vec::new());

impl J {
    fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(e) => e.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            J::Str(s) => Some(s),
            _ => None,
        }
    }

    fn entries(&self) -> &[(String, J)] {
        match self {
            J::Obj(e) => e,
            _ => &[],
        }
    }

    fn strings(&self, key: &str) -> Vec<&str> {
        match self.get(key) {
            Some(J::Arr(a)) => a
                .iter()
                .filter_map(|v| match v {
                    J::Str(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn is_nonempty_obj(&self) -> bool {
        matches!(self, J::Obj(e) if !e.is_empty())
    }
}

fn s(v: &str) -> J {
    J::Str(v.to_owned())
}

fn obj<const N: usize>(entries: [(&str, J); N]) -> J {
    J::Obj(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_bool<E>(self, v: bool) -> Result<J, E> {
                Ok(J::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<J, E> {
                Ok(J::Num(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<J, E> {
                Ok(J::Num(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<J, E> {
                Number::from_f64(v)
                    .map(J::Num)
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E>(self, v: &str) -> Result<J, E> {
                Ok(J::Str(v.to_owned()))
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element()? {
                    out.push(v);
                }
                Ok(J::Arr(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut out = Vec::new();
                while let Some(kv) = a.next_entry()? {
                    out.push(kv);
                }
                Ok(J::Obj(out))
            }
        }
        d.deserialize_any(V)
    }
}

impl Serialize for J {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            J::Null => ser.serialize_unit(),
            J::Bool(b) => ser.serialize_bool(*b),
            J::Num(n) => n.serialize(ser),
            J::Str(x) => ser.serialize_str(x),
            J::Arr(a) => a.serialize(ser),
            J::Obj(e) => {
                let mut m = ser.serialize_map(Some(e.len()))?;
                for (k, v) in e {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
    }
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    let mut after_lower = false;
    for c in name.chars() {
        if c.is_ascii_uppercase() && after_lower {
            out.push('-');
        }
        after_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn resolve<'a>(spec: &'a J, mut node: &'a J) -> &'a J {
    while let Some(r) = node.str("$ref") {
        node = r
            .trim_start_matches(['#', '/'])
            .split('/')
            .fold(spec, |n, p| n.get(p).unwrap_or(&J::Null));
    }
    node
}

fn description(node: &J) -> Option<&str> {
    node.str("description").filter(|d| !d.is_empty())
}

/// Display type, kind, and item kind for a schema property.
fn type_of(spec: &J, prop: &J) -> (String, &'static str, Option<&'static str>) {
    let prop = resolve(spec, prop);
    match prop.str("type").unwrap_or("object") {
        "boolean" => ("boolean".into(), "bool", None),
        "integer" => ("integer".into(), "int", None),
        "number" if matches!(prop.str("format"), Some("int32" | "int64")) => {
            ("integer".into(), "int", None)
        }
        "number" => ("number".into(), "float", None),
        "string" => ("string".into(), "string", None),
        "array" => {
            let (item_type, item_kind, _) = type_of(spec, prop.get("items").unwrap_or(&EMPTY));
            if item_kind == "json" {
                ("array<object>".into(), "json", None)
            } else {
                (format!("array<{item_type}>"), "list", Some(item_kind))
            }
        }
        _ => ("object".into(), "json", None),
    }
}

fn params_for(spec: &J, schema: &J) -> Vec<J> {
    let schema = resolve(spec, schema);
    let required = schema.strings("required");
    schema
        .get("properties")
        .unwrap_or(&EMPTY)
        .entries()
        .iter()
        .map(|(name, prop)| {
            let (display, kind, item) = type_of(spec, prop);
            let mut p = vec![
                ("name", s(name)),
                ("flag", s(&kebab(name))),
                ("type", s(&display)),
                ("kind", s(kind)),
                ("required", J::Bool(required.contains(&name.as_str()))),
            ];
            if let Some(item) = item {
                p.push(("item", s(item)));
            }
            if let Some(d) = prop.get("default") {
                p.push(("default", d.clone()));
            }
            if let Some(d) = description(prop).or_else(|| description(resolve(spec, prop))) {
                p.push(("description", s(d)));
            }
            J::Obj(p.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
        })
        .collect()
}

/// Describes a response schema as dotted paths, e.g. `domains[].dnsSummary.passesMx`.
fn flatten(spec: &J, schema: &J, prefix: &str, out: &mut Vec<J>, seen: &[Option<&str>]) {
    let schema = resolve(spec, schema);
    let required = schema.strings("required");
    for (name, raw) in schema.get("properties").unwrap_or(&EMPTY).entries() {
        let path = format!("{prefix}{name}");
        let prop = resolve(spec, raw);
        let (display, _, _) = type_of(spec, prop);
        let mut entry = vec![
            ("path", s(&path)),
            ("type", s(&display)),
            ("required", J::Bool(required.contains(&name.as_str()))),
        ];
        if let Some(d) = description(raw).or_else(|| description(prop)) {
            entry.push(("description", s(d)));
        }
        out.push(J::Obj(
            entry.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        ));
        let r = raw
            .str("$ref")
            .or_else(|| prop.get("items").and_then(|i| i.str("$ref")));
        if seen.contains(&r) {
            continue;
        }
        let seen: Vec<_> = seen.iter().copied().chain([r]).collect();
        if prop.str("type") == Some("array")
            && let Some(items) = prop.get("items").filter(|i| i.is_nonempty_obj())
        {
            let items = resolve(spec, items);
            if items.get("properties").is_some_and(J::is_nonempty_obj) {
                flatten(spec, items, &format!("{path}[]."), out, &seen);
            }
        } else if prop.get("properties").is_some_and(J::is_nonempty_obj) {
            flatten(spec, prop, &format!("{path}."), out, &seen);
        }
    }
}

fn single_required_scalar(params: &[J]) -> Option<&str> {
    let required: Vec<&J> = params
        .iter()
        .filter(|p| p.get("required") == Some(&J::Bool(true)))
        .collect();
    match required.as_slice() {
        [p] if matches!(p.str("kind"), Some("string" | "int")) => p.str("name"),
        _ => None,
    }
}

/// The hand-written `domain dns` command, listed beside the generated ones so
/// discovery shows it in the same place. The CLI supplies the implementation.
fn dns_command() -> J {
    let field = |path: &str, ty: &str, required: bool, desc: &str| {
        obj([
            ("path", s(path)),
            ("type", s(ty)),
            ("required", J::Bool(required)),
            ("description", s(desc)),
        ])
    };
    obj([
        ("name", s("dns")),
        ("operation", s("getOwnershipCode")),
        ("method", s("POST")),
        ("path", s("/api/v0/getOwnershipCode")),
        (
            "summary",
            s("Print the DNS records a domain needs for Purelymail"),
        ),
        (
            "description",
            s(
                "Lists MX, SPF, DKIM (3 CNAMEs), DMARC and the ownership TXT record for a domain, \
               ready to feed to a DNS provider. Calls getOwnershipCode for the ownership value; \
               --skip-ownership works offline.",
            ),
        ),
        ("destructive", J::Bool(false)),
        ("custom", J::Bool(true)),
        ("positional", s("domain")),
        (
            "params",
            J::Arr(vec![obj([
                ("name", s("domain")),
                ("flag", s("domain")),
                ("type", s("string")),
                ("kind", s("string")),
                ("required", J::Bool(true)),
                (
                    "description",
                    s("Domain the records are for, e.g. example.com"),
                ),
            ])]),
        ),
        (
            "returns",
            J::Arr(vec![
                field("[].type", "string", true, "MX, TXT or CNAME"),
                field("[].name", "string", true, "Fully qualified record name"),
                field("[].content", "string", true, "Record value"),
                field("[].priority", "integer", false, "MX priority"),
                field("[].purpose", "string", true, "What the record is for"),
            ]),
        ),
    ])
}

fn build(spec: &J) -> Result<J, String> {
    let mut by_resource: Vec<(&str, Vec<J>)> =
        RESOURCES.iter().map(|(n, _)| (*n, Vec::new())).collect();
    let mut unmapped = Vec::new();
    for (path, methods) in spec.get("paths").unwrap_or(&EMPTY).entries() {
        for (method, op) in methods.entries() {
            let api_name = path.rsplit('/').next().unwrap_or(path);
            let Some((_, resource, friendly)) = OPS.iter().find(|(o, _, _)| *o == api_name) else {
                unmapped.push(api_name);
                continue;
            };
            let body = op
                .get("requestBody")
                .map(|b| resolve(spec, b))
                .and_then(|b| b.get("content")?.get("application/json")?.get("schema"))
                .ok_or(format!("{api_name}: no JSON request body"))?;
            let params = params_for(spec, body);
            let mut returns = Vec::new();
            if let Some(ok) = op.get("responses").and_then(|r| {
                r.get("200")?
                    .get("content")?
                    .get("application/json")?
                    .get("schema")
            }) && let Some(result) = resolve(spec, ok)
                .get("properties")
                .and_then(|p| p.get("result"))
            {
                flatten(spec, result, "", &mut returns, &[]);
            }
            let positional = single_required_scalar(&params).map(s);
            let mut entry = vec![
                ("name", s(friendly)),
                ("operation", s(api_name)),
                ("method", s(&method.to_uppercase())),
                ("path", s(path)),
                ("summary", s(op.str("summary").unwrap_or("").trim())),
                ("description", s(op.str("description").unwrap_or("").trim())),
                ("destructive", J::Bool(api_name.starts_with("delete"))),
                ("params", J::Arr(params)),
                ("returns", J::Arr(returns)),
            ];
            if let Some(p) = positional {
                entry.push(("positional", p));
            }
            let ops = &mut by_resource
                .iter_mut()
                .find(|(n, _)| n == resource)
                .unwrap()
                .1;
            ops.push(J::Obj(
                entry.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            ));
        }
    }
    if !unmapped.is_empty() {
        unmapped.sort_unstable();
        return Err(format!(
            "unmapped operations (add them to OPS): {}",
            unmapped.join(", ")
        ));
    }
    let found: Vec<&str> = by_resource
        .iter()
        .flat_map(|(_, ops)| ops.iter().filter_map(|o| o.str("operation")))
        .collect();
    let mut missing: Vec<&str> = OPS
        .iter()
        .map(|(o, _, _)| *o)
        .filter(|o| !found.contains(o))
        .collect();
    if !missing.is_empty() {
        missing.sort_unstable();
        return Err(format!(
            "operations no longer in the spec: {}",
            missing.join(", ")
        ));
    }

    let resources = RESOURCES
        .iter()
        .zip(by_resource)
        .map(|((name, desc), (_, mut ops))| {
            if *name == "domain" {
                ops.push(dns_command());
            }
            obj([
                ("name", s(name)),
                ("description", s(desc)),
                ("ops", J::Arr(ops)),
            ])
        })
        .collect();
    let info = spec.get("info").unwrap_or(&EMPTY);
    let scheme = spec
        .get("components")
        .and_then(|c| c.get("securitySchemes"))
        .and_then(|s| s.entries().first())
        .map(|(_, v)| v)
        .ok_or("spec has no securitySchemes")?;
    Ok(obj([
        (
            "api",
            obj([
                ("title", s(info.str("title").unwrap_or(""))),
                ("version", s(info.str("version").unwrap_or(""))),
                ("base_url", s(BASE_URL)),
                ("auth_header", s(scheme.str("name").unwrap_or(""))),
                ("docs_url", s(DOCS_URL)),
                ("spec_url", s(SPEC_URL)),
            ]),
        ),
        ("resources", J::Arr(resources)),
    ]))
}

/// One-space indentation and a trailing newline, the format the schema has always had.
fn render(tree: &J) -> String {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        serde_json::ser::PrettyFormatter::with_indent(b" "),
    );
    tree.serialize(&mut ser)
        .expect("serializing to a Vec cannot fail");
    buf.push(b'\n');
    String::from_utf8(buf).expect("serde_json writes UTF-8")
}

/// The published file is `window.swaggerSpec=<json>;`; plain JSON is accepted too.
fn strip_wrapper(text: &str) -> &str {
    let t = text.trim();
    let t = t.strip_prefix("window.swaggerSpec=").unwrap_or(t);
    t.trim().trim_end_matches(';').trim()
}

fn fetch(source: &str) -> Result<String, String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        ureq::get(source)
            .call()
            .and_then(|mut r| r.body_mut().read_to_string())
            .map_err(|e| format!("fetch {source}: {e}"))
    } else {
        std::fs::read_to_string(source).map_err(|e| format!("read {source}: {e}"))
    }
}

fn run(source: &str) -> Result<String, String> {
    let text = fetch(source)?;
    let spec: J =
        serde_json::from_str(strip_wrapper(&text)).map_err(|e| format!("parse {source}: {e}"))?;
    let tree = build(&spec)?;
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/schema.json");
    std::fs::write(&out, render(&tree)).map_err(|e| format!("write {}: {e}", out.display()))?;
    let resources = match tree.get("resources") {
        Some(J::Arr(r)) => r,
        _ => unreachable!("build always emits resources"),
    };
    let ops: usize = resources
        .iter()
        .map(|r| match r.get("ops") {
            Some(J::Arr(o)) => o.len(),
            _ => 0,
        })
        .sum();
    Ok(format!(
        "wrote src/schema.json: {} resources, {ops} operations",
        resources.len()
    ))
}

fn main() -> ExitCode {
    let source = std::env::args()
        .nth(1)
        .unwrap_or_else(|| SPEC_URL.to_owned());
    match run(&source) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("gen: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Follows a `/`-separated path of keys and array indexes.
    fn at<'a>(j: &'a J, path: &str) -> &'a J {
        path.split('/')
            .fold(j, |n, p| match (n, p.parse::<usize>()) {
                (J::Arr(a), Ok(i)) => &a[i],
                _ => n.get(p).unwrap_or_else(|| panic!("no {p} in {path}")),
            })
    }

    /// A spec with every mapped operation stubbed, plus a real body for
    /// createRoutingRule and a nested response for listDomains.
    fn fixture(extra_path: &str) -> J {
        let mut paths = Vec::new();
        for (op, _, _) in OPS {
            let (body, response) = match *op {
                "createRoutingRule" => (
                    r##"{"$ref":"#/components/requestBodies/Routing"}"##,
                    r##"{"$ref":"#/components/schemas/Empty"}"##,
                ),
                "listDomains" => (
                    r##"{"$ref":"#/components/requestBodies/Empty"}"##,
                    r##"{"$ref":"#/components/schemas/ListDomains"}"##,
                ),
                _ => (
                    r##"{"$ref":"#/components/requestBodies/Empty"}"##,
                    r##"{"$ref":"#/components/schemas/Empty"}"##,
                ),
            };
            paths.push(format!(
                r#""/api/v0/{op}":{{"post":{{"summary":"Does {op}.","description":" Detail. ","requestBody":{body},"responses":{{"200":{{"content":{{"application/json":{{"schema":{{"type":"object","properties":{{"result":{response}}}}}}}}}}}}}}}}}"#
            ));
        }
        if !extra_path.is_empty() {
            paths.push(extra_path.to_owned());
        }
        let paths = paths.join(",");
        let text = format!(
            r##"{{"info":{{"title":"T","version":"9"}},"servers":[{{"url":"https://ignored.example"}}],
            "paths":{{{paths}}},
            "components":{{
              "securitySchemes":{{"token":{{"name":"X-Token","type":"apiKey","in":"header"}}}},
              "requestBodies":{{
                "Empty":{{"content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/Empty"}}}}}}}},
                "Routing":{{"content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/Routing"}}}}}}}}
              }},
              "schemas":{{
                "Empty":{{"type":"object","properties":{{}}}},
                "Routing":{{"type":"object","required":["domainName","targetAddresses","ruleId"],"properties":{{
                  "domainName":{{"type":"string"}},
                  "targetAddresses":{{"type":"array","items":{{"type":"string"}},"description":"Where"}},
                  "ruleId":{{"type":"number","format":"int64"}},
                  "catchall":{{"type":"boolean","default":false}}
                }}}},
                "ListDomains":{{"type":"object","required":["domains"],"properties":{{"domains":{{"type":"array","items":{{"$ref":"#/components/schemas/Domain"}}}}}}}},
                "Domain":{{"type":"object","required":["name"],"properties":{{"name":{{"type":"string"}},"dns":{{"$ref":"#/components/schemas/Dns","description":"Summary"}}}}}},
                "Dns":{{"type":"object","properties":{{"passesMx":{{"type":"boolean"}}}}}}
              }}
            }}}}"##
        );
        serde_json::from_str(&text).expect("fixture parses")
    }

    #[test]
    fn kebab_splits_before_uppercase_after_lowercase_or_digit() {
        assert_eq!(kebab("userName"), "user-name");
        assert_eq!(
            kebab("requireTwoFactorAuthentication"),
            "require-two-factor-authentication"
        );
        assert_eq!(kebab("ip4Address"), "ip4-address");
        assert_eq!(kebab("name"), "name");
    }

    #[test]
    fn builds_the_tree_shape() {
        let tree = build(&fixture("")).unwrap();
        assert_eq!(at(&tree, "api/base_url"), &s(BASE_URL));
        assert_eq!(at(&tree, "api/auth_header"), &s("X-Token"));
        let names: Vec<_> = (0..6)
            .map(|i| at(&tree, &format!("resources/{i}/name")).clone())
            .collect();
        assert_eq!(
            names,
            RESOURCES.iter().map(|(n, _)| s(n)).collect::<Vec<_>>()
        );

        let create = at(&tree, "resources/3/ops/0");
        assert_eq!(create.str("operation"), Some("createRoutingRule"));
        assert_eq!(create.str("summary"), Some("Does createRoutingRule."));
        assert_eq!(create.str("description"), Some("Detail."));
        assert_eq!(
            create.get("positional"),
            None,
            "three required fields, no positional"
        );
        let targets = at(create, "params/1");
        assert_eq!(targets.str("flag"), Some("target-addresses"));
        assert_eq!(targets.str("type"), Some("array<string>"));
        assert_eq!(targets.str("kind"), Some("list"));
        assert_eq!(targets.str("item"), Some("string"));
        assert_eq!(targets.get("required"), Some(&J::Bool(true)));
        assert_eq!(targets.str("description"), Some("Where"));
        assert_eq!(at(create, "params/2").str("kind"), Some("int"));
        assert_eq!(at(create, "params/3").get("default"), Some(&J::Bool(false)));

        let delete = at(&tree, "resources/4/ops/4");
        assert_eq!(delete.str("operation"), Some("deleteDomain"));
        assert_eq!(delete.get("destructive"), Some(&J::Bool(true)));
        assert_eq!(delete.get("params"), Some(&J::Arr(vec![])));

        let list = at(&tree, "resources/4/ops/1");
        assert_eq!(list.str("operation"), Some("listDomains"));
        let paths: Vec<_> = (0..4)
            .map(|i| at(list, &format!("returns/{i}/path")).clone())
            .collect();
        assert_eq!(
            paths,
            [
                "domains",
                "domains[].name",
                "domains[].dns",
                "domains[].dns.passesMx"
            ]
            .map(s)
        );
        assert_eq!(at(list, "returns/2").str("description"), Some("Summary"));
        assert_eq!(at(list, "returns/2").str("type"), Some("object"));

        let dns = at(&tree, "resources/4/ops/5");
        assert_eq!(dns.str("name"), Some("dns"));
        assert_eq!(dns.get("custom"), Some(&J::Bool(true)));
    }

    #[test]
    fn rejects_an_operation_with_no_row() {
        let spec = fixture(
            r##""/api/v0/launchRocket":{"post":{"requestBody":{"$ref":"#/components/requestBodies/Empty"},"responses":{}}}"##,
        );
        assert_eq!(
            build(&spec).unwrap_err(),
            "unmapped operations (add them to OPS): launchRocket"
        );
    }

    #[test]
    fn rejects_a_spec_missing_a_mapped_operation() {
        let mut spec = fixture("");
        if let Some(J::Obj(paths)) = spec_paths(&mut spec) {
            paths.retain(|(p, _)| !p.ends_with("/listUser") && !p.ends_with("/addDomain"));
        }
        assert_eq!(
            build(&spec).unwrap_err(),
            "operations no longer in the spec: addDomain, listUser"
        );
    }

    fn spec_paths(spec: &mut J) -> Option<&mut J> {
        match spec {
            J::Obj(e) => e.iter_mut().find(|(k, _)| k == "paths").map(|(_, v)| v),
            _ => None,
        }
    }

    #[test]
    fn renders_one_space_indent_with_trailing_newline() {
        let tree = obj([("a", J::Arr(vec![])), ("b", obj([("c", J::Bool(true))]))]);
        assert_eq!(
            render(&tree),
            "{\n \"a\": [],\n \"b\": {\n  \"c\": true\n }\n}\n"
        );
    }

    #[test]
    fn strips_the_js_wrapper() {
        assert_eq!(
            strip_wrapper("window.swaggerSpec={\"a\":1}\n;\n"),
            "{\"a\":1}"
        );
        assert_eq!(strip_wrapper(" {\"a\":1} "), "{\"a\":1}");
    }
}
