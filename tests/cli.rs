//! Behaviour tests against the built binary. API calls go to a mock server
//! through `PURELYMAIL_API_URL`; everything else runs offline.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use mockito::{Mock, ServerGuard};
use serde_json::{Value, json};

const TOKEN: &str = "pm-live-test-token";

struct Run {
    out: String,
    err: String,
    code: i32,
}

/// Runs the CLI with no real config file, a token, and the mock server as
/// the API. `env` overrides or (with an empty value) unsets a variable.
fn run(server: Option<&ServerGuard>, env: &[(&str, &str)], args: &[&str]) -> Run {
    run_stdin(server, env, args, "")
}

fn run_stdin(
    server: Option<&ServerGuard>,
    env: &[(&str, &str)],
    args: &[&str],
    stdin: &str,
) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_purelymail"));
    cmd.env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", std::env::temp_dir())
        .env("PURELYMAIL_CONFIG", "/nonexistent/purelymail.toml")
        .env("PURELYMAIL_API_TOKEN", TOKEN)
        .env(
            "PURELYMAIL_API_URL",
            server.map_or("http://127.0.0.1:1".to_owned(), |s| s.url()),
        );
    for (k, v) in env {
        if v.is_empty() {
            cmd.env_remove(k);
        } else {
            cmd.env(k, v);
        }
    }
    let mut child = cmd
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    Run {
        out: String::from_utf8(output.stdout).unwrap(),
        err: String::from_utf8(output.stderr).unwrap(),
        code: output.status.code().unwrap(),
    }
}

fn mock(server: &mut ServerGuard, operation: &str, status: usize, body: &str) -> Mock {
    server
        .mock("POST", format!("/api/v0/{operation}").as_str())
        .with_status(status)
        .with_body(body)
        .expect(1)
        .create()
}

fn dry_run_body(args: &[&str]) -> Value {
    let mut argv = vec!["--dry-run"];
    argv.extend(args);
    let r = run(None, &[], &argv);
    assert_eq!(r.code, 0, "stderr: {}", r.err);
    serde_json::from_str::<Value>(&r.out).unwrap()["body"].take()
}

#[test]
fn success_envelope_prints_result_and_sends_token_and_body() {
    let mut server = mockito::Server::new();
    let m = server
        .mock("POST", "/api/v0/listDomains")
        .match_header("Purelymail-Api-Token", TOKEN)
        .match_header("Content-Type", "application/json")
        .match_body(r#"{"includeShared":true}"#)
        .with_body(r#"{"type":"success","result":{"domains":[{"name":"a.com"}]}}"#)
        .expect(1)
        .create();
    let r = run(Some(&server), &[], &["domain", "list", "--include-shared"]);
    m.assert();
    assert_eq!(
        (r.code, r.out.as_str(), r.err.as_str()),
        (0, "{\"domains\":[{\"name\":\"a.com\"}]}\n", "")
    );
}

#[test]
fn success_without_result_prints_empty_object() {
    let mut server = mockito::Server::new();
    mock(&mut server, "addDomain", 200, r#"{"type":"success"}"#);
    let r = run(Some(&server), &[], &["domain", "add", "example.com"]);
    assert_eq!((r.code, r.out.as_str()), (0, "{}\n"));
}

#[test]
fn invalid_token_exits_2_with_code_and_message_on_stderr() {
    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "listDomains",
        200,
        r#"{"type":"error","code":"invalidToken","message":"Token not valid."}"#,
    );
    let r = run(Some(&server), &[], &["domain", "list"]);
    assert_eq!(
        (r.code, r.out.as_str(), r.err.as_str()),
        (2, "", "error: invalidToken: Token not valid.\n")
    );
}

#[test]
fn other_api_error_exits_3() {
    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "getUser",
        200,
        r#"{"type":"error","code":"internalError","message":"No such user"}"#,
    );
    let r = run(Some(&server), &[], &["user", "get", "nobody@example.com"]);
    assert_eq!(
        (r.code, r.out.as_str(), r.err.as_str()),
        (3, "", "error: internalError: No such user\n")
    );
}

#[test]
fn list_operation_retries_429_then_succeeds() {
    let mut server = mockito::Server::new();
    let throttled = server
        .mock("POST", "/api/v0/listUser")
        .with_status(429)
        .with_header("Retry-After", "0")
        .expect(1)
        .create();
    let ok = mock(
        &mut server,
        "listUser",
        200,
        r#"{"type":"success","result":{"users":["a@b.c"]}}"#,
    );
    let r = run(Some(&server), &[], &["user", "list"]);
    throttled.assert();
    ok.assert();
    assert_eq!((r.code, r.out.as_str()), (0, "{\"users\":[\"a@b.c\"]}\n"));
}

#[test]
fn rate_limit_after_retries_exits_4() {
    let mut server = mockito::Server::new();
    let m = server
        .mock("POST", "/api/v0/listUser")
        .with_status(429)
        .with_header("Retry-After", "0")
        .expect(2)
        .create();
    let r = run(Some(&server), &[], &["user", "list", "--max-retries", "1"]);
    m.assert();
    assert_eq!(
        (r.code, r.err.as_str()),
        (4, "error: http 429: rate limited\n")
    );
}

#[test]
fn create_operation_is_never_retried() {
    let mut server = mockito::Server::new();
    let m = mock(&mut server, "createUser", 503, "upstream down");
    let r = run(
        Some(&server),
        &[],
        &[
            "user",
            "create",
            "--user-name",
            "a",
            "--domain-name",
            "b.c",
            "--password",
            "x",
        ],
    );
    m.assert();
    assert_eq!(
        (r.code, r.out.as_str(), r.err.as_str()),
        (5, "", "error: http 503: upstream down\n")
    );
}

#[test]
fn non_200_html_body_exits_5_with_its_first_line() {
    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "listDomains",
        404,
        "<!DOCTYPE html>\n<html>not found</html>",
    );
    let r = run(
        Some(&server),
        &[],
        &["domain", "list", "--max-retries", "0"],
    );
    assert_eq!(
        (r.code, r.err.as_str()),
        (5, "error: http 404: <!DOCTYPE html>\n")
    );
}

#[test]
fn a_200_without_an_envelope_exits_5() {
    let mut server = mockito::Server::new();
    mock(&mut server, "listDomains", 200, r#"{"domains":[]}"#);
    let r = run(Some(&server), &[], &["domain", "list"]);
    assert_eq!(r.code, 5, "stderr: {}", r.err);
    assert!(
        r.err
            .starts_with("error: http 200 but not a Purelymail API envelope: {\"domains\":[]}"),
        "{}",
        r.err
    );
    assert_eq!(r.out, "");
}

#[test]
fn delete_is_refused_without_yes_and_nothing_is_sent() {
    let mut server = mockito::Server::new();
    let m = server
        .mock("POST", "/api/v0/deleteDomain")
        .expect(0)
        .create();
    let r = run(Some(&server), &[], &["domain", "delete", "example.com"]);
    m.assert();
    assert_eq!((r.code, r.out.as_str()), (1, ""));
    assert!(r.err.contains("re-run with --yes"), "{}", r.err);

    let r = run(
        Some(&server),
        &[],
        &["raw", "deleteDomain", "--body", r#"{"name":"example.com"}"#],
    );
    m.assert();
    assert_eq!(r.code, 1);
}

#[test]
fn delete_with_yes_is_sent() {
    let mut server = mockito::Server::new();
    let m = mock(&mut server, "deleteDomain", 200, r#"{"type":"success"}"#);
    let r = run(
        Some(&server),
        &[],
        &["domain", "delete", "example.com", "--yes"],
    );
    m.assert();
    assert_eq!((r.code, r.out.as_str()), (0, "{}\n"));
}

#[test]
fn missing_token_exits_1_but_discovery_and_dry_run_work_without_one() {
    let no_token = [("PURELYMAIL_API_TOKEN", "")];
    let r = run(None, &no_token, &["domain", "list"]);
    assert_eq!(r.code, 1);
    assert!(r.err.starts_with("error: no API token"), "{}", r.err);

    let r = run(None, &no_token, &["list"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.starts_with("RESOURCE"));

    let r = run(None, &no_token, &["domain", "list", "--dry-run"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out.contains(r#""Purelymail-Api-Token":"<none>""#),
        "{}",
        r.out
    );
}

#[test]
fn bad_command_is_a_usage_error_before_any_token_lookup() {
    let r = run(
        None,
        &[
            ("PURELYMAIL_API_TOKEN", ""),
            ("PURELYMAIL_CONFIG", "/dev/null/not-a-dir"),
        ],
        &["bogus"],
    );
    assert_eq!(r.code, 1);
    assert!(
        r.err.contains("unrecognized subcommand 'bogus'"),
        "{}",
        r.err
    );

    let r = run(None, &[], &["domain", "list", "--no-such-flag"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err.contains("unexpected argument '--no-such-flag'"),
        "{}",
        r.err
    );
}

#[test]
fn dry_run_redacts_the_token_to_its_length() {
    let r = run(None, &[], &["--dry-run", "account", "credit"]);
    assert_eq!(r.code, 0);
    assert!(!r.out.contains(TOKEN));
    assert!(
        r.out.contains(&format!(
            "\"Purelymail-Api-Token\":\"<redacted, {} chars>\"",
            TOKEN.len()
        )),
        "{}",
        r.out
    );
    assert_eq!(
        serde_json::from_str::<Value>(&r.out).unwrap()["url"],
        "http://127.0.0.1:1/api/v0/checkAccountCredit"
    );
}

#[test]
fn verbose_logs_the_request_without_the_token() {
    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "checkAccountCredit",
        200,
        r#"{"type":"success","result":{"credit":"1.00"}}"#,
    );
    let r = run(Some(&server), &[], &["-v", "account", "credit"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.err,
        format!(
            "purelymail: POST {}/api/v0/checkAccountCredit\n",
            server.url()
        )
    );
}

#[test]
fn flags_coerce_to_the_body_field_types() {
    assert_eq!(
        dry_run_body(&[
            "user",
            "create",
            "--user-name",
            "a",
            "--domain-name",
            "b.c",
            "--password",
            "x",
            "--send-welcome-email=false",
            "--enable-search-indexing"
        ]),
        json!({"userName": "a", "domainName": "b.c", "password": "x", "sendWelcomeEmail": false, "enableSearchIndexing": true})
    );
    let rule = [
        "routing",
        "create",
        "--domain-name",
        "b.c",
        "--prefix=false",
        "--match-user",
        "h",
    ];
    let targets = json!(["a@x.com", "b@x.com"]);
    for extra in [
        vec![
            "--target-addresses",
            "a@x.com",
            "--target-addresses",
            "b@x.com",
        ],
        vec!["--target-addresses", "a@x.com, b@x.com"],
        vec!["--target-addresses", r#"["a@x.com","b@x.com"]"#],
    ] {
        let mut args = rule.to_vec();
        args.extend(extra);
        assert_eq!(dry_run_body(&args)["targetAddresses"], targets, "{args:?}");
    }
    assert_eq!(
        dry_run_body(&["routing", "delete", "42"]),
        json!({"routingRuleId": 42})
    );
    assert_eq!(
        dry_run_body(&["routing", "delete", "--routing-rule-id", "7"]),
        json!({"routingRuleId": 7})
    );
}

#[test]
fn json_array_flag_with_the_wrong_item_type_is_a_usage_error() {
    let r = run(
        None,
        &[],
        &[
            "--dry-run",
            "routing",
            "create",
            "--domain-name",
            "b.c",
            "--prefix",
            "--match-user",
            "h",
            "--target-addresses",
            r#"["a@x.com", 5]"#,
        ],
    );
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err,
        "error: --target-addresses expects array<string>, got 5\n"
    );
}

#[test]
fn body_is_merged_and_explicit_flags_win() {
    assert_eq!(
        dry_run_body(&[
            "user",
            "modify",
            "--body",
            r#"{"userName":"old@b.c","newPassword":"p","extra":1}"#,
            "--user-name",
            "new@b.c"
        ]),
        json!({"userName": "new@b.c", "newPassword": "p", "extra": 1})
    );
    let r = run_stdin(
        None,
        &[],
        &["--dry-run", "domain", "list", "-f", "-"],
        r#"{"includeShared": true}"#,
    );
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        serde_json::from_str::<Value>(&r.out).unwrap()["body"],
        json!({"includeShared": true})
    );
}

#[test]
fn positional_conflicts_with_its_flag_and_required_fields_are_checked() {
    let r = run(
        None,
        &[],
        &[
            "--dry-run",
            "domain",
            "add",
            "a.com",
            "--domain-name",
            "b.com",
        ],
    );
    assert_eq!(
        (r.code, r.err.as_str()),
        (
            1,
            "error: domainName given both as an argument and as --domain-name\n"
        )
    );
    let r = run(
        None,
        &[],
        &["--dry-run", "user", "create", "--user-name", "a"],
    );
    assert_eq!(
        (r.code, r.err.as_str()),
        (
            1,
            "error: user create: missing required --domain-name, --password (`purelymail describe user create`)\n"
        )
    );
}

/// A config file under target/tmp, so `cargo clean` removes it.
fn config_file(name: &str, contents: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.toml"));
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn token_precedence_is_flag_then_env_then_file() {
    let path = config_file(
        "precedence",
        "api_token = \"file-token-default\"\n\n[client-b]\napi_token = \"file-token-b\"\n",
    );
    let file = path.to_str().unwrap();
    let show = |env: &[(&str, &str)], args: &[&str]| -> Value {
        let mut argv = vec!["config", "show", "-o", "json"];
        argv.extend(args);
        let r = run(None, env, &argv);
        assert_eq!(r.code, 0, "{}", r.err);
        for secret in ["flag-token", TOKEN, "file-token"] {
            assert!(!r.out.contains(secret), "token leaked: {}", r.out);
        }
        serde_json::from_str(&r.out).unwrap()
    };
    let v = show(&[("PURELYMAIL_CONFIG", file)], &["--token", "flag-token"]);
    assert_eq!(
        (v["token_source"].as_str(), v["api_token"].as_str()),
        (Some("--token"), Some("set (10 chars)"))
    );
    let v = show(&[("PURELYMAIL_CONFIG", file)], &[]);
    assert_eq!(
        (v["token_source"].as_str(), v["api_token"].as_str()),
        (Some("PURELYMAIL_API_TOKEN"), Some("set (18 chars)"))
    );
    let v = show(
        &[("PURELYMAIL_CONFIG", file), ("PURELYMAIL_API_TOKEN", "")],
        &[],
    );
    assert_eq!(v["token_source"], format!("{file} [default]"));
    assert_eq!(v["api_token"], "set (18 chars)");
    let v = show(
        &[("PURELYMAIL_CONFIG", file), ("PURELYMAIL_API_TOKEN", "")],
        &["--profile", "client-b"],
    );
    assert_eq!(
        (
            v["token_source"].as_str(),
            v["profile"].as_str(),
            v["api_token"].as_str()
        ),
        (
            Some(format!("{file} [client-b]").as_str()),
            Some("client-b"),
            Some("set (12 chars)")
        )
    );
    let v = show(
        &[
            ("PURELYMAIL_CONFIG", file),
            ("PURELYMAIL_API_TOKEN", ""),
            ("PURELYMAIL_PROFILE", "client-b"),
        ],
        &[],
    );
    assert_eq!(v["profile"], "client-b");

    let r = run(
        None,
        &[("PURELYMAIL_CONFIG", file)],
        &["--profile", "nope", "config", "show"],
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("profile \"nope\" not found"), "{}", r.err);

    let r = run(
        None,
        &[("PURELYMAIL_CONFIG", file), ("PURELYMAIL_API_TOKEN", "")],
        &["--dry-run", "--profile", "client-b", "account", "credit"],
    );
    assert!(r.out.contains("<redacted, 12 chars>"), "{}", r.out);
}

#[test]
fn config_show_text_has_no_token() {
    let r = run(None, &[], &["config", "show"]);
    assert_eq!(r.code, 0);
    assert!(!r.out.contains(TOKEN));
    assert!(
        r.out.contains("api_token     set (18 chars)\n"),
        "{}",
        r.out
    );
    assert!(
        r.out.contains("token_source  PURELYMAIL_API_TOKEN\n"),
        "{}",
        r.out
    );
}

#[test]
fn config_verify_exit_codes() {
    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "checkAccountCredit",
        200,
        r#"{"type":"success","result":{"credit":"12.50"}}"#,
    );
    let r = run(Some(&server), &[], &["config", "verify"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let v: Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(
        (
            v["ok"].as_bool(),
            v["credit"].as_str(),
            v["token_source"].as_str()
        ),
        (Some(true), Some("12.50"), Some("PURELYMAIL_API_TOKEN"))
    );

    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "checkAccountCredit",
        200,
        r#"{"type":"error","code":"invalidToken","message":"Token not valid."}"#,
    );
    assert_eq!(run(Some(&server), &[], &["config", "verify"]).code, 2);

    let mut server = mockito::Server::new();
    mock(&mut server, "checkAccountCredit", 503, "down");
    let r = run(
        Some(&server),
        &[],
        &["config", "verify", "--max-retries", "0"],
    );
    assert_eq!(
        (r.code, r.err.as_str()),
        (1, "error: could not verify the token: http 503: down\n")
    );

    assert_eq!(
        run(None, &[("PURELYMAIL_API_TOKEN", "")], &["config", "verify"]).code,
        1
    );
}

#[test]
fn describe_and_tree_come_from_the_embedded_schema() {
    let r = run(None, &[], &["describe", "routing", "create", "-o", "json"]);
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(v["command"], "purelymail routing create");
    assert_eq!(v["url"], "https://purelymail.com/api/v0/createRoutingRule");
    let targets = &v["op"]["params"][3];
    assert_eq!(
        (
            targets["flag"].as_str(),
            targets["kind"].as_str(),
            targets["item"].as_str(),
            targets["required"].as_bool()
        ),
        (
            Some("target-addresses"),
            Some("list"),
            Some("string"),
            Some(true)
        )
    );

    let r = run(None, &[], &["describe", "addDomain"]);
    assert_eq!(r.code, 0);
    assert!(
        r.out
            .starts_with("POST https://purelymail.com/api/v0/addDomain  (addDomain)\n"),
        "{}",
        r.out
    );
    assert!(
        r.out.contains("  purelymail domain add <domainName>\n"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .contains("domainName may be given as the first argument instead of --domain-name."),
        "{}",
        r.out
    );
    assert!(
        r.out.contains("--domain-name  domainName  string  yes"),
        "{}",
        r.out
    );

    let r = run(None, &[], &["describe", "domain", "delete"]);
    assert!(
        r.out.contains("Destructive: refuses to run without --yes"),
        "{}",
        r.out
    );

    let r = run(None, &[], &["tree"]);
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(v["api"]["base_url"], "https://purelymail.com");
    assert_eq!(v["resources"].as_array().unwrap().len(), 6);
    assert_eq!(v["resources"][4]["ops"][5]["name"], "dns");
    assert_eq!(v["resources"][4]["ops"][5]["custom"], true);
    assert!(
        r.out.starts_with("{\"api\":{\"title\""),
        "struct field order is kept: {}",
        &r.out[..40]
    );

    let r = run(None, &[], &["list", "domain", "-o", "json"]);
    let v: Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(v[0]["command"], "purelymail domain add");
    assert_eq!(v[4]["destructive"], true);

    let r = run(None, &[], &["list", "domian"]);
    assert_eq!(
        (r.code, r.err.as_str()),
        (
            1,
            "error: unknown resource \"domian\"; did you mean: domain\n"
        )
    );
}

#[test]
fn domain_dns_without_ownership_is_offline() {
    let r = run(
        None,
        &[("PURELYMAIL_API_TOKEN", "")],
        &["domain", "dns", "Example.COM.", "--skip-ownership"],
    );
    assert_eq!(r.code, 0, "{}", r.err);
    let v: Value = serde_json::from_str(&r.out).unwrap();
    let recs = v.as_array().unwrap();
    assert_eq!(recs.len(), 6);
    assert_eq!(
        recs[0],
        json!({"type": "MX", "name": "example.com", "content": "mailserver.purelymail.com", "priority": 50, "purpose": "mail delivery"})
    );
    assert_eq!(
        (recs[1]["type"].as_str(), recs[1]["content"].as_str()),
        (Some("TXT"), Some("v=spf1 include:_spf.purelymail.com ~all"))
    );
    for i in 1..=3 {
        let r = &recs[1 + i];
        assert_eq!(
            (
                r["type"].as_str(),
                r["name"].as_str(),
                r["content"].as_str()
            ),
            (
                Some("CNAME"),
                Some(format!("purelymail{i}._domainkey.example.com").as_str()),
                Some(format!("key{i}.dkimroot.purelymail.com").as_str())
            )
        );
    }
    assert_eq!(
        (recs[5]["name"].as_str(), recs[5]["content"].as_str()),
        (Some("_dmarc.example.com"), Some("dmarcroot.purelymail.com"))
    );

    let r = run(
        None,
        &[],
        &[
            "domain",
            "dns",
            "example.com",
            "--skip-ownership",
            "-o",
            "table",
        ],
    );
    assert!(r.out.starts_with("TYPE   NAME                                CONTENT                                  PRIORITY  PURPOSE        NOTE\nMX     example.com"), "{}", r.out);
    assert_eq!(r.out.lines().count(), 7);
}

#[test]
fn domain_dns_adds_the_ownership_record_from_the_api() {
    let mut server = mockito::Server::new();
    mock(
        &mut server,
        "getOwnershipCode",
        200,
        r#"{"type":"success","result":{"code":"abc123"}}"#,
    );
    let r = run(Some(&server), &[], &["domain", "dns", "example.com"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let v: Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 7);
    assert_eq!(
        v[2],
        json!({"type": "TXT", "name": "example.com", "content": "purelymail_ownership_proof=abc123", "purpose": "ownership proof (required by addDomain)"})
    );
}

#[test]
fn output_formats_render_list_results_as_rows() {
    let mut server = mockito::Server::new();
    let body = r#"{"type":"success","result":{"rules":[{"id":2,"matchUser":"a","targetAddresses":["x@y.z"]},{"id":3,"matchUser":"b","catchall":true}]}}"#;
    let m = server
        .mock("POST", "/api/v0/listRoutingRules")
        .with_body(body)
        .expect(4)
        .create();
    let r = run(Some(&server), &[], &["routing", "list", "-o", "table"]);
    assert_eq!(
        r.out,
        "ID  MATCHUSER  TARGETADDRESSES  CATCHALL\n2   a          [\"x@y.z\"]        \n3   b                           true\n"
    );
    let r = run(
        Some(&server),
        &[],
        &["routing", "list", "-o", "csv", "--columns", "matchUser,id"],
    );
    assert_eq!(r.out, "matchUser,id\na,2\nb,3\n");
    let r = run(Some(&server), &[], &["routing", "list", "-o", "ndjson"]);
    assert_eq!(
        r.out,
        "{\"id\":2,\"matchUser\":\"a\",\"targetAddresses\":[\"x@y.z\"]}\n{\"catchall\":true,\"id\":3,\"matchUser\":\"b\"}\n"
    );
    let r = run(Some(&server), &[], &["routing", "list", "--pretty"]);
    assert!(
        r.out
            .starts_with("{\n  \"rules\": [\n    {\n      \"id\": 2,"),
        "{}",
        r.out
    );
    m.assert();
}

#[test]
fn config_file_errors_never_quote_the_file() {
    let unquoted = config_file("unquoted", "api_token = pm-live-raw-secret\n");
    let r = run(
        None,
        &[
            ("PURELYMAIL_CONFIG", unquoted.to_str().unwrap()),
            ("PURELYMAIL_API_TOKEN", ""),
        ],
        &["config", "show"],
    );
    assert_eq!(r.code, 1, "{}", r.err);
    assert!(r.err.contains("invalid TOML at line 1"), "{}", r.err);
    assert!(!r.err.contains("raw-secret"), "{}", r.err);

    let wrong_key = config_file("wrong-key", "token = \"pm-live-other-secret\"\n");
    let r = run(
        None,
        &[
            ("PURELYMAIL_CONFIG", wrong_key.to_str().unwrap()),
            ("PURELYMAIL_API_TOKEN", ""),
        ],
        &["config", "show"],
    );
    assert_eq!(r.code, 1, "{}", r.err);
    assert!(r.err.contains("unknown key under \"token\""), "{}", r.err);
    assert!(!r.err.contains("other-secret"), "{}", r.err);
}

#[test]
fn body_values_may_start_with_a_hyphen() {
    let body = dry_run_body(&[
        "user",
        "create",
        "--user-name",
        "a",
        "--domain-name",
        "b.c",
        "--password",
        "-x1",
    ]);
    assert_eq!(body["password"], "-x1");
}

#[test]
fn list_operation_retries_502_then_succeeds() {
    let mut server = mockito::Server::new();
    let bad = mock(&mut server, "listDomains", 502, "bad gateway");
    let ok = mock(
        &mut server,
        "listDomains",
        200,
        r#"{"type":"success","result":{"domains":[]}}"#,
    );
    let r = run(Some(&server), &[], &["domain", "list"]);
    bad.assert();
    ok.assert();
    assert_eq!((r.code, r.out.as_str()), (0, "{\"domains\":[]}\n"));
}

#[test]
fn list_operation_does_not_retry_500() {
    let mut server = mockito::Server::new();
    let m = mock(&mut server, "listDomains", 500, "boom");
    let r = run(Some(&server), &[], &["domain", "list"]);
    m.assert();
    assert_eq!((r.code, r.err.as_str()), (5, "error: http 500: boom\n"));
}

#[test]
fn unknown_envelope_type_exits_5() {
    for body in [r#"{"type":"weird"}"#, r#"{"type":5}"#] {
        let mut server = mockito::Server::new();
        let m = mock(&mut server, "checkAccountCredit", 200, body);
        let r = run(Some(&server), &[], &["account", "credit"]);
        m.assert();
        assert_eq!(r.code, 5, "{body}: {}", r.err);
        assert!(r.err.contains("not a Purelymail API envelope"), "{}", r.err);
    }
}
