//! Builds the clap command tree from the schema and runs the matched command.

use std::io::{self, Read, Write};

use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::api::{Client, Error};
use crate::config::{self, Config};
use crate::output::{self, FORMATS, Format};
use crate::schema::{Kind, Op, Param, Resource, Schema, suggest};

/// Flags this tool owns on every command. A body field with one of these
/// names gets no flag of its own and stays reachable through `--body`.
const RESERVED: [&str; 13] = [
    "body",
    "file",
    "yes",
    "profile",
    "token",
    "output",
    "pretty",
    "columns",
    "dry-run",
    "verbose",
    "max-retries",
    "help",
    "version",
];

/// Arg id of the optional first argument; flags are kebab-case so it cannot collide.
const POSITIONAL: &str = "_positional";

pub fn command(schema: &Schema) -> Command {
    let mut root = Command::new("purelymail")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A command line interface to the Purelymail API")
        .long_about(
            "purelymail exposes the Purelymail API as commands, generated from Purelymail's own \
             OpenAPI spec so every operation and body field is covered.\n\n\
             Start with `purelymail list` to see the resources and `purelymail describe \
             <resource> <op>` to see what an operation takes. Output is JSON by default.",
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .args(global_args());
    for res in &schema.resources {
        root = root.subcommand(resource_command(res));
    }
    root.subcommand(raw_command())
        .subcommand(
            Command::new("list")
                .about("List resources, or the operations on one resource")
                .arg(Arg::new("resource").value_name("resource")),
        )
        .subcommand(
            Command::new("describe")
                .about("Show the fields an operation accepts and what it returns")
                .long_about(
                    "Prints an operation's endpoint, every body field with its flag, type, \
                     whether it is required, its API default and description, and the shape \
                     of the result. Accepts the API's own name too: `describe addDomain`.",
                )
                .arg(Arg::new("resource").value_name("resource").required(true))
                .arg(Arg::new("operation").value_name("operation")),
        )
        .subcommand(Command::new("tree").about("Dump the command tree as JSON"))
        .subcommand(
            Command::new("config")
                .about("Inspect profiles and credentials")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("show").about("Show the resolved settings, never the token"),
                )
                .subcommand(
                    Command::new("verify")
                        .about("Check the token against the API (read-only)")
                        .long_about(
                            "Calls checkAccountCredit, which is read-only, and reports the \
                             account credit.\n\nExit 0 when the token works, 1 when no token is \
                             configured or the API could not be reached, 2 when Purelymail \
                             refused the token.",
                        ),
                ),
        )
}

fn global_args() -> Vec<Arg> {
    let flag = |name: &'static str, help: &'static str| {
        Arg::new(name)
            .long(name)
            .help(help)
            .action(ArgAction::SetTrue)
            .global(true)
    };
    vec![
        Arg::new("profile")
            .long("profile")
            .value_name("name")
            .help("Profile from the config file to use")
            .global(true),
        Arg::new("token")
            .long("token")
            .value_name("token")
            .help("API token (overrides PURELYMAIL_API_TOKEN)")
            .global(true),
        Arg::new("output")
            .short('o')
            .long("output")
            .value_name("format")
            .value_parser(FORMATS)
            .default_value("json")
            .help("Output format")
            .global(true),
        flag("pretty", "Indent JSON output"),
        Arg::new("columns")
            .long("columns")
            .value_name("a,b,c")
            .value_delimiter(',')
            .action(ArgAction::Append)
            .help("Column order for table and csv output")
            .global(true),
        flag(
            "dry-run",
            "Print the request that would be sent, without sending it",
        ),
        flag("verbose", "Log requests to stderr").short('v'),
        Arg::new("max-retries")
            .long("max-retries")
            .value_name("n")
            .value_parser(value_parser!(u32))
            .default_value("2")
            .help("Retries for a throttled or unavailable API, on read operations only")
            .global(true),
    ]
}

fn body_args() -> [Arg; 2] {
    [
        Arg::new("body")
            .long("body")
            .value_name("json")
            .help("JSON object used as the request body; explicit flags win"),
        Arg::new("file")
            .short('f')
            .long("file")
            .value_name("path")
            .conflicts_with("body")
            .help("Read the request body from a JSON file (- for stdin)"),
    ]
}

fn yes_arg(help: &'static str) -> Arg {
    Arg::new("yes")
        .long("yes")
        .action(ArgAction::SetTrue)
        .help(help)
}

fn resource_command(res: &Resource) -> Command {
    let mut cmd = Command::new(res.name.clone())
        .about(res.description.clone())
        .subcommand_required(true)
        .arg_required_else_help(true);
    for op in &res.ops {
        cmd = cmd.subcommand(if op.custom {
            custom_command(op)
        } else {
            op_command(res, op)
        });
    }
    cmd
}

fn op_command(res: &Resource, op: &Op) -> Command {
    let mut short = op.summary.trim_end_matches('.').to_owned();
    if op.destructive {
        short.push_str(" (destructive, needs --yes)");
    }
    let mut cmd = Command::new(op.name.clone())
        .about(short)
        .long_about(format!(
            "{} {} ({})\n\n{}\n{}\nEvery body field is a flag below; --body/--file supply a \
             whole JSON body.\n`purelymail describe {} {}` shows the fields and the result shape.",
            op.method,
            op.path,
            op.operation,
            if op.description.is_empty() {
                &op.summary
            } else {
                &op.description
            },
            if op.destructive {
                "\nDestructive: refuses to run without --yes (or --dry-run to preview).\n"
            } else {
                ""
            },
            res.name,
            op.name,
        ))
        .args(body_args());
    if op.destructive {
        cmd = cmd.arg(yes_arg("Confirm this destructive operation"));
    }
    if let Some(pos) = &op.positional {
        cmd = cmd.arg(Arg::new(POSITIONAL).value_name(pos.clone()).help(format!(
            "Same as --{}",
            op.param(pos).map_or(pos.as_str(), |p| &p.flag)
        )));
    }
    for p in op
        .params
        .iter()
        .filter(|p| !RESERVED.contains(&p.flag.as_str()))
    {
        cmd = cmd.arg(param_arg(p));
    }
    cmd
}

fn param_arg(p: &Param) -> Arg {
    let mut help = Vec::new();
    if p.required {
        help.push("(required)".to_owned());
    }
    help.push(p.description.clone().unwrap_or_else(|| p.ty.clone()));
    if p.kind == Kind::List {
        help.push("-- repeat the flag, use commas, or pass a JSON array".to_owned());
    }
    if let Some(d) = &p.default {
        help.push(format!("[API default {}]", output::cell(Some(d))));
    }
    let arg = Arg::new(p.flag.clone())
        .long(p.flag.clone())
        .help(help.join(" "));
    match p.kind {
        Kind::Bool => arg
            .value_name("true|false")
            .value_parser(value_parser!(bool))
            .num_args(0..=1)
            .default_missing_value("true")
            .require_equals(true),
        Kind::List => arg.value_name("value").action(ArgAction::Append),
        Kind::Int => arg.value_name("n").value_parser(value_parser!(i64)),
        Kind::Float => arg.value_name("x").value_parser(value_parser!(f64)),
        Kind::String | Kind::Json => arg.value_name(p.name.clone()),
    }
}

fn custom_command(op: &Op) -> Command {
    Command::new(op.name.clone())
        .about(op.summary.clone())
        .long_about(format!(
            "{}\n\nValues follow https://purelymail.com/docs/domainDocs. JSON by default, an \
             array of {{type, name, content, priority?, purpose, note?}} with fully qualified \
             names; -o table for reading.",
            op.description
        ))
        .arg(Arg::new("domain").value_name("domain").required(true))
        .arg(
            Arg::new("skip-ownership")
                .long("skip-ownership")
                .action(ArgAction::SetTrue)
                .help("Leave out the ownership TXT record and do not call the API"),
        )
}

fn raw_command() -> Command {
    Command::new("raw")
        .about("Call any Purelymail API operation directly")
        .long_about(
            "POSTs a JSON body to https://purelymail.com/api/v0/<operation> with the configured \
             token and prints the unwrapped result.\n\nThe operation is the API's own camelCase \
             name (addDomain, listUser, ...). Operations whose name starts with \"delete\" need \
             --yes, like their generated twins.",
        )
        .arg(Arg::new("operation").value_name("operation").required(true))
        .args(body_args())
        .arg(yes_arg("Confirm a destructive (delete*) operation"))
}

/// The global flags, read once from the root matches.
struct Globals {
    profile: Option<String>,
    token: Option<String>,
    out: output::Options,
    format_explicit: bool,
    dry_run: bool,
    verbose: bool,
    max_retries: u32,
}

impl Globals {
    fn from(m: &ArgMatches) -> Globals {
        let format = m.get_one::<String>("output").expect("has a default");
        Globals {
            profile: m.get_one::<String>("profile").cloned(),
            token: m.get_one::<String>("token").cloned(),
            out: output::Options {
                format: Format::parse(format).expect("clap validated the format"),
                pretty: m.get_flag("pretty"),
                columns: m
                    .get_many::<String>("columns")
                    .map(|c| c.cloned().collect())
                    .unwrap_or_default(),
            },
            format_explicit: m.value_source("output") == Some(ValueSource::CommandLine),
            dry_run: m.get_flag("dry-run"),
            verbose: m.get_flag("verbose"),
            max_retries: *m.get_one::<u32>("max-retries").expect("has a default"),
        }
    }

    /// Discovery output defaults to readable text and switches to structured
    /// output when a format is asked for explicitly.
    fn structured(&self) -> bool {
        self.format_explicit && self.out.format != Format::Table
    }

    fn config(&self) -> Result<Config, Error> {
        config::resolve(self.profile.as_deref(), self.token.as_deref())
    }

    fn client(&self, schema: &Schema, cfg: &Config) -> Result<Client, Error> {
        if cfg.token.is_none() && !self.dry_run {
            return Err(Error::NoToken);
        }
        Ok(Client::new(
            &base_url(schema),
            cfg.token.clone(),
            self.max_retries,
            self.dry_run,
            self.verbose,
        ))
    }

    fn render(&self, v: &Value) -> Result<(), Error> {
        output::write(&mut io::stdout().lock(), &self.out, v)?;
        Ok(())
    }

    /// Structured discovery output. Structs keep their field order as JSON;
    /// other formats go through the generic value renderer.
    fn emit(&self, v: &impl Serialize) -> Result<(), Error> {
        if self.out.format == Format::Json {
            output::write_json(&mut io::stdout().lock(), v, self.out.pretty)?;
            return Ok(());
        }
        self.render(&serde_json::to_value(v).map_err(io::Error::other)?)
    }
}

/// `PURELYMAIL_API_URL` exists for tests and proxies. It is not a flag,
/// because pointing a token at another host should take more than a typo.
fn base_url(schema: &Schema) -> String {
    std::env::var("PURELYMAIL_API_URL")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| schema.api.base_url.clone())
}

pub fn run(schema: &Schema, m: &ArgMatches) -> Result<(), Error> {
    let g = Globals::from(m);
    let (name, sub) = m.subcommand().expect("subcommand_required");
    match name {
        "list" => list(schema, &g, sub.get_one::<String>("resource")),
        "describe" => describe(schema, &g, sub),
        "tree" => g.emit(schema),
        "config" => config_command(schema, &g, sub),
        "raw" => raw(schema, &g, sub),
        _ => {
            let res = schema
                .resource(name)
                .expect("clap only accepts schema resources");
            let (op_name, sub) = sub.subcommand().expect("subcommand_required");
            let op = res.op(op_name).expect("clap only accepts schema ops");
            match (res.name.as_str(), op.name.as_str()) {
                ("domain", "dns") => dns(schema, &g, op, sub),
                _ => operation(schema, &g, res, op, sub),
            }
        }
    }
}

fn operation(
    schema: &Schema,
    g: &Globals,
    res: &Resource,
    op: &Op,
    m: &ArgMatches,
) -> Result<(), Error> {
    let mut body = load_body(m)?;
    for p in &op.params {
        if RESERVED.contains(&p.flag.as_str())
            || m.value_source(&p.flag) != Some(ValueSource::CommandLine)
        {
            continue;
        }
        body.insert(p.name.clone(), flag_value(p, m)?);
    }
    if let Some(pos) = op.positional.as_deref()
        && let Some(given) = m.get_one::<String>(POSITIONAL)
    {
        let p = op.param(pos).expect("positional names a param");
        if m.value_source(&p.flag) == Some(ValueSource::CommandLine) {
            return Err(Error::Usage(format!(
                "{} given both as an argument and as --{}",
                p.name, p.flag
            )));
        }
        body.insert(p.name.clone(), positional_value(p, given)?);
    }
    let mut missing: Vec<String> = op
        .params
        .iter()
        .filter(|p| p.required && !body.contains_key(&p.name))
        .map(|p| format!("--{}", p.flag))
        .collect();
    if !missing.is_empty() {
        missing.sort();
        return Err(Error::Usage(format!(
            "{} {}: missing required {} (`purelymail describe {} {}`)",
            res.name,
            op.name,
            missing.join(", "),
            res.name,
            op.name
        )));
    }
    if op.destructive && !m.get_flag("yes") && !g.dry_run {
        return Err(Error::Usage(format!(
            "{} {} is destructive ({}); re-run with --yes to confirm, or --dry-run to preview",
            res.name, op.name, op.description
        )));
    }
    let client = g.client(schema, &g.config()?)?;
    g.render(&client.call(&op.operation, &body)?)
}

fn flag_value(p: &Param, m: &ArgMatches) -> Result<Value, Error> {
    let one = |id: &str| {
        m.get_one::<String>(id)
            .expect("value_source said it is set")
    };
    Ok(match p.kind {
        Kind::Bool => Value::Bool(*m.get_one::<bool>(&p.flag).expect("set")),
        Kind::Int => json!(*m.get_one::<i64>(&p.flag).expect("set")),
        Kind::Float => json!(*m.get_one::<f64>(&p.flag).expect("set")),
        Kind::String => Value::String(one(&p.flag).clone()),
        Kind::Json => serde_json::from_str(one(&p.flag)).map_err(|_| {
            Error::Usage(format!(
                "--{} expects JSON ({}), got: {}",
                p.flag,
                p.ty,
                one(&p.flag)
            ))
        })?,
        Kind::List => {
            let values = m.get_many::<String>(&p.flag).expect("set");
            list_value(p, values.map(String::as_str))?
        }
    })
}

/// A value starting with `[` is a JSON array; anything else splits on commas.
fn list_value<'a>(p: &Param, values: impl Iterator<Item = &'a str>) -> Result<Value, Error> {
    let item = p.item.unwrap_or(Kind::String);
    let mut items = Vec::new();
    for v in values.map(str::trim) {
        if v.starts_with('[') {
            let parsed: Vec<Value> = serde_json::from_str(v).map_err(|_| {
                Error::Usage(format!(
                    "--{} looks like a JSON array but does not parse: {v}",
                    p.flag
                ))
            })?;
            for x in parsed {
                if !matches_kind(&x, item) {
                    return Err(Error::Usage(format!(
                        "--{} expects {}, got {x}",
                        p.flag, p.ty
                    )));
                }
                items.push(x);
            }
        } else {
            for part in v.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                items.push(scalar(part, item).ok_or_else(|| {
                    Error::Usage(format!("--{} expects {}, got {part:?}", p.flag, p.ty))
                })?);
            }
        }
    }
    Ok(Value::Array(items))
}

fn matches_kind(v: &Value, kind: Kind) -> bool {
    match kind {
        Kind::String => v.is_string(),
        Kind::Int => v.is_i64(),
        Kind::Float => v.is_number(),
        Kind::Bool => v.is_boolean(),
        Kind::Json | Kind::List => true,
    }
}

fn scalar(s: &str, kind: Kind) -> Option<Value> {
    match kind {
        Kind::Int => s.parse::<i64>().ok().map(Value::from),
        Kind::Float => s.parse::<f64>().ok().map(Value::from),
        Kind::Bool => s.parse::<bool>().ok().map(Value::Bool),
        Kind::String | Kind::Json | Kind::List => Some(Value::String(s.to_owned())),
    }
}

fn positional_value(p: &Param, given: &str) -> Result<Value, Error> {
    if p.kind == Kind::Int {
        return given
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| Error::Usage(format!("{} expects an integer, got {given:?}", p.name)));
    }
    Ok(Value::String(given.to_owned()))
}

/// The base request body from `--body` or `--file`; `-` reads stdin.
fn load_body(m: &ArgMatches) -> Result<Map<String, Value>, Error> {
    let raw = match (m.get_one::<String>("body"), m.get_one::<String>("file")) {
        (Some(inline), _) => inline.clone(),
        (None, Some(f)) if f == "-" => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            s
        }
        (None, Some(f)) => std::fs::read_to_string(f)
            .map_err(|e| Error::Usage(format!("read body from {f}: {e}")))?,
        (None, None) => return Ok(Map::new()),
    };
    if raw.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(Value::Null) => Ok(Map::new()),
        Ok(_) => Err(Error::Usage(
            "request body must be a JSON object".to_owned(),
        )),
        Err(e) => Err(Error::Usage(format!(
            "request body must be a JSON object: {e}"
        ))),
    }
}

fn raw(schema: &Schema, g: &Globals, m: &ArgMatches) -> Result<(), Error> {
    let given = m.get_one::<String>("operation").expect("required");
    let operation = given.trim_matches('/');
    if operation.is_empty() || operation.contains(['/', '?', '#', ' ']) {
        return Err(Error::Usage(format!(
            "operation must be a bare name like listDomains, got {given:?}"
        )));
    }
    let body = load_body(m)?;
    if schema.by_operation(operation).is_none() {
        eprintln!("purelymail: {operation} is not in the generated tree; sending it anyway");
    }
    if operation.to_lowercase().starts_with("delete") && !m.get_flag("yes") && !g.dry_run {
        return Err(Error::Usage(format!(
            "{operation} is destructive; re-run with --yes to confirm, or --dry-run to preview"
        )));
    }
    let client = g.client(schema, &g.config()?)?;
    g.render(&client.call(operation, &body)?)
}

fn resource_or_suggest<'a>(schema: &'a Schema, name: &str) -> Result<&'a Resource, Error> {
    if let Some(res) = schema.resource(name) {
        return Ok(res);
    }
    let names = schema.resource_names();
    let hints = suggest(name, &names, 5);
    Err(Error::Usage(if hints.is_empty() {
        format!("unknown resource {name:?} (have: {})", names.join(", "))
    } else {
        format!(
            "unknown resource {name:?}; did you mean: {}",
            hints.join(", ")
        )
    }))
}

fn list(schema: &Schema, g: &Globals, resource: Option<&String>) -> Result<(), Error> {
    if let Some(name) = resource {
        return list_ops(g, resource_or_suggest(schema, name)?);
    }
    if g.structured() {
        let rows: Vec<Value> = schema
            .resources
            .iter()
            .map(|r| json!({"name": r.name, "description": r.description, "operations": r.op_names()}))
            .collect();
        return g.render(&Value::Array(rows));
    }
    let mut lines = vec![
        ["RESOURCE", "OPERATIONS", "DESCRIPTION"]
            .map(str::to_owned)
            .to_vec(),
    ];
    lines.extend(schema.resources.iter().map(|r| {
        vec![
            r.name.clone(),
            r.op_names().join(" "),
            r.description.clone(),
        ]
    }));
    print!("{}", output::aligned(&lines));
    eprintln!(
        "\n`purelymail list <resource>` shows its operations; `purelymail describe <resource> \
         <op>` what one takes."
    );
    Ok(())
}

fn list_ops(g: &Globals, res: &Resource) -> Result<(), Error> {
    if g.structured() {
        let rows: Vec<Value> = res
            .ops
            .iter()
            .map(|o| {
                json!({
                    "name": o.name,
                    "command": format!("purelymail {} {}", res.name, o.name),
                    "operation": o.operation,
                    "summary": o.summary,
                    "destructive": o.destructive,
                    "custom": o.custom,
                    "params": o.params.len(),
                })
            })
            .collect();
        return g.render(&Value::Array(rows));
    }
    let mut lines = vec![
        ["COMMAND", "API OPERATION", "SUMMARY"]
            .map(str::to_owned)
            .to_vec(),
    ];
    lines.extend(res.ops.iter().map(|o| {
        let mut summary = o.summary.clone();
        if o.destructive {
            summary.push_str(" [destructive: --yes]");
        }
        let mut operation = o.operation.clone();
        if o.custom {
            operation.push_str(" (+local)");
        }
        vec![format!("{} {}", res.name, o.name), operation, summary]
    }));
    print!("{}", output::aligned(&lines));
    Ok(())
}

#[derive(Serialize)]
struct Describe<'a> {
    command: String,
    op: &'a Op,
    resource: &'a str,
    url: String,
}

fn describe(schema: &Schema, g: &Globals, m: &ArgMatches) -> Result<(), Error> {
    let first = m.get_one::<String>("resource").expect("required");
    let (res, op) = match m.get_one::<String>("operation") {
        None => match schema.by_operation(first) {
            Some(found) => found,
            None => return list_ops(g, resource_or_suggest(schema, first)?),
        },
        Some(op_name) => {
            let res = resource_or_suggest(schema, first)?;
            let op = res.op(op_name).ok_or_else(|| {
                Error::Usage(format!(
                    "{} has no operation {op_name:?} (have: {})",
                    res.name,
                    res.op_names().join(", ")
                ))
            })?;
            (res, op)
        }
    };
    if g.structured() {
        return g.emit(&Describe {
            command: format!("purelymail {} {}", res.name, op.name),
            op,
            resource: &res.name,
            url: format!("{}{}", schema.api.base_url, op.path),
        });
    }
    let mut w = io::stdout().lock();
    writeln!(
        w,
        "{} {}{}  ({})",
        op.method, schema.api.base_url, op.path, op.operation
    )?;
    let desc = if op.description.is_empty() || op.description == op.summary {
        op.summary.clone()
    } else {
        format!("{}. {}", op.summary.trim_end_matches('.'), op.description)
    };
    writeln!(w, "{desc}\n\n  {}", example(res, op))?;
    if op.destructive {
        writeln!(
            w,
            "\n  Destructive: refuses to run without --yes (--dry-run previews it)."
        )?;
    }
    if let Some(p) = op.positional.as_deref().and_then(|pos| op.param(pos)) {
        writeln!(
            w,
            "\n  {} may be given as the first argument instead of --{}.",
            p.name, p.flag
        )?;
    }
    writeln!(w)?;
    if op.params.is_empty() {
        writeln!(w, "No body fields (sends {{}}).")?;
    } else {
        let label = if op.custom { "ARGUMENT" } else { "BODY FIELD" };
        let mut lines = vec![
            ["FLAG", label, "TYPE", "REQUIRED", "DEFAULT", "DESCRIPTION"]
                .map(str::to_owned)
                .to_vec(),
        ];
        lines.extend(op.params.iter().map(|p| {
            vec![
                format!("--{}", p.flag),
                p.name.clone(),
                p.ty.clone(),
                if p.required { "yes" } else { "no" }.to_owned(),
                match &p.default {
                    Some(Value::String(s)) if s.is_empty() => "\"\"".to_owned(),
                    Some(d) => output::cell(Some(d)),
                    None => String::new(),
                },
                p.description.clone().unwrap_or_default(),
            ]
        }));
        write!(w, "{}", output::aligned(&lines))?;
    }
    if !op.returns.is_empty() {
        writeln!(w, "\nRETURNS")?;
        let lines: Vec<Vec<String>> = op
            .returns
            .iter()
            .map(|f| {
                vec![
                    format!("  {}", f.path),
                    f.ty.clone(),
                    f.description.clone().unwrap_or_default(),
                ]
            })
            .collect();
        write!(w, "{}", output::aligned(&lines))?;
    } else if !op.custom {
        writeln!(w, "\nRETURNS  {{}} on success")?;
    }
    if !op.custom {
        writeln!(
            w,
            "\nAlso: --body '<json>' or -f file.json for the whole body (flags win), --dry-run \
             to preview."
        )?;
    }
    Ok(())
}

/// A sample invocation naming every required field.
fn example(res: &Resource, op: &Op) -> String {
    let mut parts = vec!["purelymail".to_owned(), res.name.clone(), op.name.clone()];
    for p in op.params.iter().filter(|p| p.required) {
        if op.positional.as_deref() == Some(&p.name) {
            parts.push(format!("<{}>", p.name));
        } else if p.kind == Kind::Bool {
            parts.push(format!("--{}=false", p.flag));
        } else {
            parts.push(format!("--{}", p.flag));
            parts.push(match p.kind {
                Kind::Int => "<n>".to_owned(),
                Kind::List => "<value>".to_owned(),
                _ => format!("<{}>", p.name),
            });
        }
    }
    if op.destructive {
        parts.push("--yes".to_owned());
    }
    parts.join(" ")
}

fn config_command(schema: &Schema, g: &Globals, m: &ArgMatches) -> Result<(), Error> {
    let cfg = g.config()?;
    match m.subcommand().expect("subcommand_required").0 {
        "show" => {
            let token = match &cfg.token {
                Some(t) => format!("set ({} chars)", t.chars().count()),
                None => "(not set)".to_owned(),
            };
            let view = [
                ("profile", cfg.profile.clone()),
                ("config_file", config::path().display().to_string()),
                ("api_url", base_url(schema)),
                ("api_token", token),
                ("token_source", cfg.source.clone()),
            ];
            if g.structured() {
                let map: Map<String, Value> = view
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), Value::String(v)))
                    .collect();
                return g.render(&Value::Object(map));
            }
            let lines: Vec<Vec<String>> = view
                .into_iter()
                .map(|(k, v)| vec![k.to_owned(), v])
                .collect();
            print!("{}", output::aligned(&lines));
            Ok(())
        }
        _ => {
            let client = g.client(schema, &cfg)?;
            let result = match client.call("checkAccountCredit", &Map::new()) {
                Ok(v) => v,
                Err(e @ Error::Api { .. }) if e.exit_code() == 2 => return Err(e),
                Err(e) => {
                    return Err(Error::Usage(format!("could not verify the token: {e}")));
                }
            };
            if g.dry_run {
                return g.render(&result);
            }
            g.render(&json!({
                "ok": true,
                "profile": cfg.profile,
                "token_source": cfg.source,
                "api_url": client.base_url(),
                "credit": result.get("credit"),
            }))
        }
    }
}

/// The records Purelymail needs on a custom domain, per
/// https://purelymail.com/docs/domainDocs. Field order is alphabetical so the
/// JSON matches what the table and csv renderers show.
#[derive(Serialize)]
struct DnsRecord {
    content: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u16>,
    purpose: String,
    #[serde(rename = "type")]
    ty: &'static str,
}

const OWNERSHIP_KEY: &str = "purelymail_ownership_proof=";

fn dns_records(domain: &str, ownership: Option<&str>) -> Vec<DnsRecord> {
    let rec = |ty, name: String, content: String, purpose: String, note, priority| DnsRecord {
        content,
        name,
        note,
        priority,
        purpose,
        ty,
    };
    let apex = domain.to_owned();
    let mut recs = vec![
        rec(
            "MX",
            apex.clone(),
            "mailserver.purelymail.com".to_owned(),
            "mail delivery".to_owned(),
            None,
            Some(50),
        ),
        rec(
            "TXT",
            apex.clone(),
            "v=spf1 include:_spf.purelymail.com ~all".to_owned(),
            "SPF".to_owned(),
            Some("merge into an existing SPF record rather than adding a second one"),
            None,
        ),
    ];
    if let Some(code) = ownership {
        recs.push(rec(
            "TXT",
            apex,
            code.to_owned(),
            "ownership proof (required by addDomain)".to_owned(),
            None,
            None,
        ));
    }
    for i in 1..=3 {
        recs.push(rec(
            "CNAME",
            format!("purelymail{i}._domainkey.{domain}"),
            format!("key{i}.dkimroot.purelymail.com"),
            format!("DKIM key {i}"),
            Some("DNS only (not proxied) on Cloudflare"),
            None,
        ));
    }
    recs.push(rec(
        "CNAME",
        format!("_dmarc.{domain}"),
        "dmarcroot.purelymail.com".to_owned(),
        "DMARC".to_owned(),
        Some("DNS only (not proxied) on Cloudflare; replaces any existing _dmarc TXT"),
        None,
    ));
    recs
}

fn dns(schema: &Schema, g: &Globals, op: &Op, m: &ArgMatches) -> Result<(), Error> {
    let given = m.get_one::<String>("domain").expect("required");
    let domain = given.trim().to_lowercase();
    let domain = domain.trim_end_matches('.');
    if domain.is_empty() || domain.contains([' ', '/', '@']) || !domain.contains('.') {
        return Err(Error::Usage(format!(
            "{given:?} does not look like a domain name"
        )));
    }
    let ownership = if m.get_flag("skip-ownership") {
        None
    } else if g.dry_run {
        Some("(dry run: value comes from getOwnershipCode)".to_owned())
    } else {
        let client = g.client(schema, &g.config()?)?;
        let result = client
            .call(&op.operation, &Map::new())
            .map_err(|e| match e {
                Error::Api { code, message } => Error::Api {
                    code,
                    message: format!(
                        "{message} (use --skip-ownership to omit the ownership record)"
                    ),
                },
                other => other,
            })?;
        let code = result
            .get("code")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| {
                Error::Transport(format!("getOwnershipCode returned no code: {result}"))
            })?;
        // The spec calls it "the DNS record value", so it is used as-is when it
        // already carries the prefix and given one otherwise.
        Some(if code.starts_with(OWNERSHIP_KEY) {
            code.to_owned()
        } else {
            format!("{OWNERSHIP_KEY}{code}")
        })
    };
    let records = dns_records(domain, ownership.as_deref());
    let mut opts = output::Options {
        format: g.out.format,
        pretty: g.out.pretty,
        columns: g.out.columns.clone(),
    };
    if opts.columns.is_empty() {
        opts.columns = ["type", "name", "content", "priority", "purpose", "note"]
            .map(str::to_owned)
            .to_vec();
    }
    let value = serde_json::to_value(&records).map_err(io::Error::other)?;
    output::write(&mut io::stdout().lock(), &opts, &value)?;
    Ok(())
}
