# purelymail

A command line interface to the [Purelymail](https://purelymail.com) API, generated from
Purelymail's own OpenAPI spec.

The commands are not written by hand. `tools/gen` reads the spec that Purelymail publishes
with its [API docs](https://news.purelymail.com/api/index.html) and writes `src/schema.json`,
which the binary embeds. All 19 API operations are covered, and every body field is typed,
marked required or optional, and described. When Purelymail adds an endpoint, you regenerate.

The tool is built for scripts and language models. Output is JSON by default. Discovery
commands describe every operation. Exit codes say what went wrong. `--dry-run` shows the
request without sending it. Deletes refuse to run without `--yes`.

## Install

```bash
cargo install purelymail
```

Or download a tarball for your platform from
[GitHub Releases](https://github.com/soulmachine/purelymail/releases).

## Authenticate

Create an API token in Purelymail (Settings, API tokens), then export it:

```bash
export PURELYMAIL_API_TOKEN="pm-live-..."
```

For more than one account, use profiles in `~/.config/purelymail/config.toml`:

```toml
api_token = "pm-live-..."

[client-b]
api_token = "pm-live-..."
```

```bash
purelymail --profile client-b domain list
```

`--token` beats `PURELYMAIL_API_TOKEN`, which beats the config file. `PURELYMAIL_PROFILE`
selects a profile. `PURELYMAIL_CONFIG` moves the file. To check what resolved, run:

```bash
purelymail config show       # never prints the token, only its source and length
purelymail config verify     # calls checkAccountCredit (read-only) and prints the credit
```

`config verify` exits `0` when the token works, `1` when no token is configured or the API
could not be reached, and `2` when Purelymail refused the token.

## Find your way around

```bash
purelymail list                    # resources and their operations
purelymail list domain             # each command and the API operation it calls
purelymail describe domain update  # body fields, types, required, defaults, result shape
purelymail describe addDomain      # the API's own operation names work too
purelymail tree                    # the whole catalog as JSON
```

```
$ purelymail describe routing create
POST https://purelymail.com/api/v0/createRoutingRule  (createRoutingRule)
Creates a new routing rule for a domain. Routing rule must not have the same user/prefix as any other existing rules for the domain.

  purelymail routing create --domain-name <domainName> --prefix=false --match-user <matchUser> --target-addresses <value>

FLAG                BODY FIELD       TYPE           REQUIRED  DEFAULT  DESCRIPTION
--domain-name       domainName       string         yes
--prefix            prefix           boolean        yes
--match-user        matchUser        string         yes                The local part of the user address to be matched, ...
--target-addresses  targetAddresses  array<string>  yes                List of full email addresses that this mail will be rerouted to
--catchall          catchall         boolean        no        false    Whether this is a 'catchall' rule that will not fire if address maps to a user
```

## Use it

| Resource | Commands | API operations |
|---|---|---|
| `user` | `create` `get` `list` `modify` `delete` | createUser getUser listUser modifyUser deleteUser |
| `password-reset` | `upsert` `list` `delete` | upsertPasswordReset listPasswordReset deletePasswordReset |
| `app-password` | `create` `delete` | createAppPassword deleteAppPassword |
| `routing` | `create` `list` `delete` | createRoutingRule listRoutingRules deleteRoutingRule |
| `domain` | `add` `list` `update` `ownership-code` `delete` `dns` | addDomain listDomains updateDomainSettings getOwnershipCode deleteDomain |
| `account` | `credit` | checkAccountCredit |

```bash
# Domains
purelymail domain list -o table
purelymail domain list --include-shared
purelymail domain ownership-code
purelymail domain add example.com
purelymail domain update example.com --recheck-dns
purelymail domain delete example.com --yes

# Users
purelymail user create --user-name hello --domain-name example.com --password '...' --send-welcome-email=false
purelymail user get hello@example.com
purelymail user modify hello@example.com --require-two-factor-authentication
purelymail app-password create hello@example.com --name "Thunderbird"

# Routing: forward hello@ to two addresses
purelymail routing create --domain-name example.com --prefix=false --match-user hello \
  --target-addresses a@gmail.com --target-addresses b@gmail.com
purelymail routing list -o table
purelymail routing delete 1234 --yes

# Anything the generated tree does not cover
purelymail raw listDomains --body '{"includeShared":true}'
```

Body fields are flags. `userName` becomes `--user-name`. Booleans take `--flag` or
`--flag=false`. Arrays take the flag repeated, comma-separated values, or a JSON array. When
an operation has exactly one required field, you can give it as the first argument
(`domain add example.com`). `--body '<json>'` or `-f body.json` (`-f -` for stdin) supplies
the whole body, and explicit flags win over it. Unset flags are never sent, so
`user modify` changes only what you name.

### DNS records for a domain

```bash
purelymail domain dns example.com            # JSON array, ready for a DNS provider's API
purelymail domain dns example.com -o table
purelymail domain dns example.com --skip-ownership   # offline, no token needed
```

The command prints the MX record (`mailserver.purelymail.com`, priority 50), the SPF record
(`v=spf1 include:_spf.purelymail.com ~all`), the three DKIM CNAMEs
(`purelymail{1,2,3}._domainkey` to `key{1,2,3}.dkimroot.purelymail.com`), the DMARC CNAME
(`_dmarc` to `dmarcroot.purelymail.com`) and the ownership TXT record from `getOwnershipCode`.
Names are fully qualified. The values follow Purelymail's
[domain docs](https://purelymail.com/docs/domainDocs). On Cloudflare the CNAMEs must be DNS
only, not proxied. Merge the SPF value into an existing SPF record rather than adding a
second one.

## Flags worth knowing

| Flag | |
|---|---|
| `--dry-run` | Print the request that would be sent. The token shows only as its length. Nothing goes out, and no token is needed. |
| `-o, --output` | `json` (default), `ndjson`, `table`, `csv`. List results render as rows. |
| `--pretty` | Indent JSON. |
| `--columns` | Column order for table and csv. |
| `--body`, `-f` | Whole request body as JSON, inline or from a file. |
| `--yes` | Required by every `delete`, and by `raw delete*`. |
| `-v, --verbose` | Log requests to stderr. |
| `--max-retries` | Retries on 429, 502, 503 and 504, for `list`, `get` and `check` operations only (default 2). A create, modify, upsert or delete is never retried, because the server may have acted on it. |
| `--profile`, `--token` | Which credential to use. |

## Exit codes

| | |
|---|---|
| `0` | success |
| `1` | usage error, missing token, or a delete refused for lack of `--yes` |
| `2` | Purelymail refused the token (`invalidToken`) |
| `3` | Purelymail rejected the request (any other `{"type":"error"}` code) |
| `4` | HTTP 429 after the retries ran out |
| `5` | transport failure: the API could not be reached, answered with a status other than 200, or sent a body that is not an API envelope |

Purelymail answers HTTP 200 for both outcomes and puts the verdict in the body
(`{"type":"success","result":{...}}` or `{"type":"error","code":"...","message":"..."}`).
On success the CLI prints only `result` to stdout. On failure it prints
`error: <code>: <message>` to stderr, leaves stdout empty, and exits with one of the codes
above.

## For AI agents

Everything an agent needs is discoverable from the binary:

1. `purelymail list -o json` lists the resources and their command names.
2. `purelymail describe <resource> <op> -o json` returns the full contract: the endpoint
   and every body field (`name`, `flag`, `type`, `kind`, `required`, `default`,
   `description`), the positional argument if any, whether it is `destructive`, and
   `returns`, the result shape as dotted paths (`domains[].dnsSummary.passesMx`).
3. `purelymail tree` returns the same for every operation in one document.
4. Before a write, run it with `--dry-run` to see the exact JSON body. It needs no token.
5. Branch on the exit code, not on stderr. stdout holds only the result.

Discovery commands print aligned text by default and JSON when `-o json` is given. API
commands print JSON by default. Deletes refuse to run without `--yes`. Do not add it
unless the user asked for that deletion.

## Regenerate the command tree

`src/schema.json` is committed, so building needs only Cargo. To regenerate it from the
live spec:

```bash
cargo run -p gen
```

`cargo run -p gen -- <path-or-url>` reads the spec from a file or another URL instead. The
generator fails if the spec gains an operation it has no command name for, so a new
endpoint cannot vanish silently. Add a row to `OPS` in `tools/gen/src/main.rs`. CI
regenerates the schema from the live spec on every run and fails when the committed file
has drifted.

## How it is put together

```
tools/gen/        Purelymail's OpenAPI spec -> src/schema.json
src/schema.rs     embeds the schema; lookup and suggestions
src/api.rs        HTTP, the response envelope, retries, the error type and its exit codes
src/dispatch.rs   schema -> clap commands, flag coercion, discovery, config, domain dns
src/config.rs     token resolution and profiles
src/output.rs     json, ndjson, table, csv
```

The code is new. The command contract, the schema format and the operation table follow
[KudcraftsHQ/purelymail-cli](https://github.com/KudcraftsHQ/purelymail-cli) (MIT).

## License

MIT for the code in this repository.

The API, and the OpenAPI spec it is generated from, belong to Purelymail. The spec is
published under a proprietary license with the
[API documentation](https://news.purelymail.com/api/index.html), so it is not
redistributed here. `cargo run -p gen` downloads it at regeneration time. The committed
schema holds only what the CLI needs to describe the API: operation names, paths, body
field names, types and their one-line descriptions. This project is not affiliated with
Purelymail.
