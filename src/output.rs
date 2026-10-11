//! Renders results as json, ndjson, table or csv.
//!
//! JSON is the default because the tool is scripted against. Table and csv
//! lay list results out as rows: a bare array, or an object whose only field
//! is an array, which is the shape of every Purelymail list result.

use std::io::{self, Write};

use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Ndjson,
    Table,
    Csv,
}

pub const FORMATS: [&str; 4] = ["json", "ndjson", "table", "csv"];

impl Format {
    pub fn parse(name: &str) -> Option<Format> {
        match name {
            "json" => Some(Format::Json),
            "ndjson" => Some(Format::Ndjson),
            "table" => Some(Format::Table),
            "csv" => Some(Format::Csv),
            _ => None,
        }
    }
}

pub struct Options {
    pub format: Format,
    pub pretty: bool,
    /// Column order for table and csv; empty means every key, in order of first appearance.
    pub columns: Vec<String>,
}

pub fn write_json(w: &mut impl Write, v: &impl Serialize, pretty: bool) -> io::Result<()> {
    if pretty {
        serde_json::to_writer_pretty(&mut *w, v)?;
    } else {
        serde_json::to_writer(&mut *w, v)?;
    }
    w.write_all(b"\n")
}

pub fn write(w: &mut impl Write, o: &Options, v: &Value) -> io::Result<()> {
    match o.format {
        Format::Json => write_json(w, v, o.pretty),
        Format::Ndjson => match records(v) {
            Some(rows) => rows.iter().try_for_each(|r| write_json(w, r, false)),
            None => write_json(w, v, false),
        },
        Format::Table => match (records(v), v) {
            (Some(rows), _) => {
                let cols = columns(&rows, &o.columns);
                let mut lines = vec![cols.iter().map(|c| c.to_uppercase()).collect()];
                lines.extend(
                    rows.iter()
                        .map(|r| cols.iter().map(|c| cell(r.get(c))).collect::<Vec<_>>()),
                );
                w.write_all(aligned(&lines).as_bytes())
            }
            (None, Value::Object(m)) => {
                let lines: Vec<Vec<String>> = m
                    .iter()
                    .map(|(k, v)| vec![k.clone(), cell(Some(v))])
                    .collect();
                w.write_all(aligned(&lines).as_bytes())
            }
            (None, _) => write_json(w, v, true),
        },
        Format::Csv => {
            let rows = records(v).or_else(|| v.as_object().map(|m| vec![m.clone()]));
            let Some(rows) = rows else {
                return write_json(w, v, false);
            };
            let cols = columns(&rows, &o.columns);
            writeln!(w, "{}", csv_line(cols.iter().map(String::as_str)))?;
            for r in &rows {
                let cells: Vec<String> = cols.iter().map(|c| cell(r.get(c))).collect();
                writeln!(w, "{}", csv_line(cells.iter().map(String::as_str)))?;
            }
            Ok(())
        }
    }
}

fn records(v: &Value) -> Option<Vec<Map<String, Value>>> {
    let items = match v {
        Value::Array(a) => a,
        Value::Object(m) if m.len() == 1 => m.values().next()?.as_array()?,
        _ => return None,
    };
    let rows = items
        .iter()
        .map(|item| match item {
            Value::Object(m) => m.clone(),
            other => Map::from_iter([("value".to_owned(), other.clone())]),
        })
        .collect();
    Some(rows)
}

/// Keys in order of first appearance, with id and name pulled to the front.
fn columns(rows: &[Map<String, Value>], explicit: &[String]) -> Vec<String> {
    if !explicit.is_empty() {
        return explicit.to_vec();
    }
    let mut cols: Vec<String> = Vec::new();
    for r in rows {
        for k in r.keys() {
            if !cols.contains(k) {
                cols.push(k.clone());
            }
        }
    }
    cols.sort_by_key(|c| match c.as_str() {
        "id" => 0,
        "name" => 1,
        _ => 2,
    });
    cols
}

/// Flattens a JSON value into one table cell.
pub fn cell(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// Columns padded to the widest cell plus two spaces. The last cell of each
/// row is written as-is, so a row never ends in padding unless a cell after
/// it is empty.
pub fn aligned(rows: &[Vec<String>]) -> String {
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0; ncols];
    for row in rows {
        for (i, c) in row.iter().enumerate().take(row.len().saturating_sub(1)) {
            widths[i] = widths[i].max(c.chars().count() + 2);
        }
    }
    let mut out = String::new();
    for row in rows {
        for (i, c) in row.iter().enumerate() {
            out.push_str(c);
            if i + 1 < row.len() {
                out.extend(std::iter::repeat_n(' ', widths[i] - c.chars().count()));
            }
        }
        out.push('\n');
    }
    out
}

fn csv_line<'a>(fields: impl Iterator<Item = &'a str>) -> String {
    fields
        .map(|f| {
            if !f.is_empty()
                && (f.contains(['"', ',', '\n', '\r']) || f.starts_with(char::is_whitespace))
            {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}
