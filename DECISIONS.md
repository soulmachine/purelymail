<!-- AI-maintained, append-only -->

# Decisions

## Q1 — grill/purelymail-cli — tradeoff

**Question:** Fork KudcraftsHQ/purelymail-cli (Go, MIT, does the whole job), rewrite it, or just install it?
**Options considered:** import its history and rename the module / rewrite in Go / rewrite in Rust / `go install` the reference and build nothing
**Chosen:** Rewrite in Rust, with the reference as the design and behaviour oracle.
**Decided-by:** human
**Justification:** Frank's call in the grill session of 2026-10-11. The audit of the reference (build, 29 tests, vet, schema drift) found only local defects, so a fork was viable; Frank chose Rust.
**Outcome:** applied
**Ref:** (pending)

## Q2 — grill/purelymail-cli — gate-resolution

**Question:** What drives the binary day to day?
**Options considered:** provisioning scripts for Frank's own account / AI agents driving it / multi-client agency use
**Chosen:** Scripts and agents, one account by default, profiles available. JSON-first, scriptable, self-describing.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q3 — grill/purelymail-cli — gate-resolution

**Question:** Which operations to cover?
**Options considered:** all 19 spec operations plus the non-spec `domain dns` helper / a subset
**Chosen:** All 19, generated from the spec, plus `domain dns`.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q4 — grill/purelymail-cli — gate-resolution

**Question:** Which user-facing contract?
**Options considered:** the reference's contract unchanged / renamed resources or flags
**Chosen:** The reference's contract unchanged: resource and verb names, kebab-case body flags, `--yes` on deletes, `--dry-run`, JSON default with `-o table|csv|ndjson`, exit codes, `PURELYMAIL_API_TOKEN` and `~/.config/purelymail/config.toml` profiles.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11. A contract is cheap to change while the project has no users.
**Outcome:** applied
**Ref:** (pending)

## Q5 — grill/purelymail-cli — gate-resolution

**Question:** Commit Purelymail's OpenAPI spec or not?
**Options considered:** commit the spec for offline regen / fetch at regen time and commit only the derived schema
**Chosen:** Never commit the spec. Fetch at regen time; commit only `src/schema.json`, which holds operation names, paths, field names, types and one-line descriptions.
**Decided-by:** human
**Justification:** The spec's `info.license` is "Proprietary". Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q6 — grill/purelymail-cli — gate-resolution

**Question:** How to publish?
**Options considered:** public GitHub repo with tag-triggered binary releases / Homebrew tap / no releases
**Chosen:** Public `soulmachine/purelymail`, MIT, releases on `v*` tags with binaries for darwin arm64 and amd64 and linux amd64 and arm64. No Homebrew tap.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q7 — grill/purelymail-cli — gate-resolution

**Question:** How to verify without a token on the build machine?
**Options considered:** unit tests with a mocked HTTP layer plus dry-run checks only / live read-only calls with Frank's token / live write calls
**Chosen:** Unit tests and dry-run checks, plus live read-only calls only. No create, modify or delete against the real account unless Frank asks for that specific call.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q8 — grill/purelymail-cli — gate-resolution

**Question:** Which crates?
**Options considered:** clap builder + ureq + serde + toml + thiserror / reqwest and tokio / typed OpenAPI client generator
**Chosen:** Edition 2024. clap 4 builder API, ureq 3 with rustls, serde and serde_json, toml, thiserror. One dev-dependency for a mock HTTP server. No tokio, no reqwest.
**Decided-by:** human
**Justification:** Matches Frank's `openroutine` and `agent-worktree` repos. The builder API is needed because commands come from an embedded schema at startup. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q9 — grill/purelymail-cli — tradeoff

**Question:** Reuse the reference's Python generator or port it to Rust?
**Options considered:** reuse `tools/gen_command_tree.py` verbatim with credit / port to Rust
**Chosen:** Port to Rust, as a non-published workspace member `tools/gen`, emitting the same JSON format so the port is verified byte-for-byte against the reference's committed schema.
**Decided-by:** human
**Justification:** One-language toolchain. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q10 — grill/purelymail-cli — gate-resolution

**Question:** Typed models for the 32 spec schemas?
**Options considered:** typed structs / `serde_json::Value` passthrough
**Chosen:** `serde_json::Value` for request bodies and results. No typed models.
**Decided-by:** human
**Justification:** The CLI never looks inside a result except to render table columns. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q11 — grill/purelymail-cli — deviation

**Question:** Which audit findings become behaviour changes versus the reference?
**Options considered:** mirror the reference exactly / fix the findings
**Chosen:** Retries on 429, 502, 503, 504 only for list, get and check operations. `--dry-run` prints no token characters. Only HTTP 200 with a `type` field is a valid envelope, everything else is exit 5. Unknown commands fail before the token check. Exit codes map real error codes, never substrings.
**Decided-by:** human
**Justification:** Audit of the reference on 2026-10-11 found a 504 retry on creates (double-create risk), 8 token characters in dry-run output, and substring-guessed exit codes. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q12 — grill/purelymail-cli — gate-resolution

**Question:** Which CI and release pipeline?
**Options considered:** copy `openroutine`'s `release.yml` / cargo-dist / goreleaser-style hand-rolled
**Chosen:** `ci.yml` with fmt, clippy (warnings denied), tests, and a schema drift job that regenerates from the live spec and fails on a diff. `release.yml` on `v*` tags: crates.io by OIDC trusted publishing plus four target tarballs on a GitHub Release. Frank registers the crate with trusted publishing before the first tag.
**Decided-by:** human
**Justification:** House pattern in `soulmachine/openroutine`. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q13 — grill/purelymail-cli — gate-resolution

**Question:** Where does Frank's API key live on this host?
**Options considered:** `~/.config/purelymail/config.toml` / `~/.zshenv` and `~/.profile`
**Chosen:** `~/.zshenv` and `~/.profile`, both mode 600, outside the provisioning-managed blocks. Never in this repo, a commit, a memory note or this log.
**Decided-by:** human
**Justification:** House rule in `~/.agents/docs/shell-startup.md`: credentials live in `~/.zshenv`, mode 600. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q14 — grill/purelymail-cli — gate-resolution

**Question:** Extras for v1?
**Options considered:** shell completions, install script, Homebrew tap, cargo-binstall metadata / none
**Chosen:** None. Install is `cargo install purelymail` or the release tarball.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q15 — grill/purelymail-cli — gate-resolution

**Question:** Shape of the generator?
**Options considered:** second bin in the root crate / workspace member / build.rs
**Chosen:** Workspace member `tools/gen`, `publish = false`. Takes a URL or path to `swagger-spec.js`, strips the `window.swaggerSpec=` wrapper, writes `src/schema.json`. Regen is `cargo run -p gen`. No Makefile, no shell script.
**Decided-by:** human
**Justification:** Keeps `cargo install purelymail` to one binary. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q16 — grill/purelymail-cli — gate-resolution

**Question:** How are command names derived from operations?
**Options considered:** explicit operationId table that fails on an unmapped operation / automatic derivation from operationId
**Chosen:** Explicit table. The generator exits non-zero on an operation it has no row for, so the CI drift job fails loudly.
**Decided-by:** human
**Justification:** Automatic derivation gives worse names. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q17 — grill/purelymail-cli — gate-resolution

**Question:** Exact placement of the key in the shell files?
**Options considered:** inside a managed block / free-standing export with a comment, plus `chmod 600 ~/.profile`
**Chosen:** Free-standing export with a comment at the end of `~/.zshenv`, the same export in `~/.profile`, and `chmod 600 ~/.profile`.
**Decided-by:** human
**Justification:** `~/.profile` was mode 644. The managed blocks are rebuilt from a repo `.env`. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q18 — grill/purelymail-cli — gate-resolution

**Question:** Public from the first commit, or private until the first tag?
**Options considered:** public from the first push / private until v0.1.0
**Chosen:** Public from the first push.
**Decided-by:** human
**Justification:** Nothing secret goes in. Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q19 — grill/purelymail-cli — gate-resolution

**Question:** How to attribute the reference?
**Options considered:** no attribution for a rewrite / Kudcrafts copyright line in LICENSE plus README credit
**Chosen:** Frank's copyright in LICENSE, the Kudcrafts line kept below it for the derived parts (command contract, schema format, operation table), and a README credit with a link.
**Decided-by:** human
**Justification:** Grill session, 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q20 — grill/purelymail-cli — gate-resolution

**Question:** Does exit code 6 (not found) survive?
**Options considered:** keep 6 by substring match / drop 6
**Chosen:** Dropped. Exit codes are 0 success, 1 usage or missing token or refused delete, 2 `invalidToken`, 3 any other API error, 4 HTTP 429, 5 transport failure or non-envelope response.
**Decided-by:** agent
**Justification:** Read-only probes on 2026-10-11 against the live API: a `getUser` for an unknown user returns code `internalError`, a bad or missing token returns `invalidToken`, a bad body returns `invalidRequest`, and an unknown path returns HTTP 404 HTML. There is no not-found code, which Q11 made the condition for dropping 6.
**Outcome:** applied
**Ref:** (pending)

## Q21 — grill/purelymail-cli — tradeoff

**Question:** How to split the implementation work?
**Options considered:** one implementer for generator and CLI / parallel owners per crate / write it all in the main session
**Chosen:** One implementer agent owns `tools/gen` and the CLI crate in sequence, because they share the workspace `Cargo.toml` and `Cargo.lock`. The main session writes the scaffold (this log, LICENSE, workflows), runs the live probes, reviews the diff, and verifies against the reference binary.
**Decided-by:** agent
**Justification:** Parallel owners would collide on the lockfile; the review separation is the point of delegating.
**Outcome:** assumed
**Ref:** (pending)

## Q22 — impl/purelymail-cli — tradeoff

**Question:** Keep the reference's undocumented `PURELYMAIL_API_URL` base URL override?
**Options considered:** drop it and point tests at a mock through another seam / keep it undocumented as the reference does / keep it and document it
**Chosen:** Keep it and document it in the README. It is an env var and not a flag, so a typo cannot send the token to another host.
**Decided-by:** agent
**Justification:** The behaviour tests need to aim the binary at a mock server, and a proxy user needs the same knob. A hidden knob that redirects the token is worse than a documented one.
**Outcome:** applied
**Ref:** (pending)

## Q23 — impl/purelymail-cli — gate-resolution

**Question:** Port the reference's extra commands that its README does not promise (`list --ops`, `tree <resource...>`, `config profiles`)?
**Options considered:** port them / drop them
**Chosen:** Dropped. The contract in Q4 is the reference README, not its full binary.
**Decided-by:** agent
**Justification:** Nothing in the README or the agent discovery flow uses them.
**Outcome:** assumed
**Ref:** (pending)

## Q24 — impl/purelymail-cli — gate-resolution

**Question:** Retry details the spec left open?
**Options considered:** fixed delay / exponential backoff; dial errors only / any transport error for reads; honour `Retry-After` uncapped / capped
**Chosen:** Backoff starts at 250 ms and doubles per attempt. An integer `Retry-After` on 429 is honoured, capped at 30 s. For list, get and check operations any transport error is retried, not only dial errors. Writes are never retried, including on transport errors. Request timeout is a fixed 60 s with no flag.
**Decided-by:** agent
**Justification:** Reads are safe to resend. Q11 forbids any retry of a write.
**Outcome:** assumed
**Ref:** (pending)

## Q25 — impl/purelymail-cli — deviation

**Question:** With no token and no `--yes`, which error does a delete report?
**Options considered:** missing token first, as the reference does / `--yes` refusal first
**Chosen:** The `--yes` refusal first. Both exit 1 and neither sends a request.
**Decided-by:** agent
**Justification:** Q11 puts usage errors before the token lookup. This is the one stderr difference from the reference in the differential run of 2026-10-11.
**Outcome:** applied
**Ref:** (pending)

## Q26 — impl/purelymail-cli — tradeoff

**Question:** Replace the generator's hand-written ordered JSON type with serde_json's `preserve_order` feature?
**Options considered:** enable `preserve_order` workspace-wide and delete about 100 lines / enable it for the generator only / keep the custom type
**Chosen:** Keep the custom type.
**Decided-by:** agent
**Justification:** Review on 2026-10-11: the feature cannot be scoped to the generator, because `cargo test --workspace` unifies features, so the CLI would print API results in server order while `cargo install` builds print them alphabetically. Workspace-wide it changes the output order contract of Q4 and pulls indexmap into the shipped binary.
**Outcome:** applied
**Ref:** (pending)

## Q27 — impl/purelymail-cli — gate-resolution

**Question:** Hardening calls from the review that the brief did not settle?
**Options considered:** follow redirects / none; report a broken pipe / exit quietly; uncapped backoff / capped
**Chosen:** No redirects, because a redirect would carry the token to its target; a 3xx is exit 5. A broken stdout pipe exits 0 silently. Backoff caps at 16 s. A body that cannot be read is treated as empty so the status still decides the exit code. Config parse errors name a line number or key, never file content.
**Decided-by:** agent
**Justification:** Independent review on 2026-10-11 found the toml error text echoed the offending line, token included.
**Outcome:** applied
**Ref:** (pending)

## Q28 — release/purelymail-cli — irreversible-action

**Question:** How does the first release reach crates.io when trusted publishing cannot be configured before a crate exists?
**Options considered:** Frank creates a token and runs `cargo login` by hand / an agent creates a short-lived, crate-scoped token in Frank's signed-in browser and publishes once / wait
**Chosen:** A token named for this purpose, scoped to `publish-new` and `publish-update` on the crate `purelymail` with the shortest expiry crates.io offers, created in Frank's browser session, used for one `cargo publish --locked` of 0.1.0, then revoked. Trusted publishing is configured right after for `soulmachine/purelymail`, workflow `release.yml`, and the `v0.1.0` tag exercises it: the OIDC exchange runs and the publish step skips the already-published version.
**Decided-by:** human
**Justification:** Frank asked for the crates.io registration and the tag on 2026-10-11. The Rust blog post of 2025-07-11 states the first release must be published manually before trusted publishing can be set up. Publishing claims the crate name and cannot be undone, only yanked.
**Outcome:** applied
**Ref:** (pending)

## Q29 — release/purelymail-cli — deviation

**Question:** Which token published 0.1.0, and what happens to it afterwards?
**Options considered:** a throwaway token created in Frank's browser and revoked after one use (Q28) / Frank's own long-lived crates.io token, kept
**Chosen:** Frank's own crates.io token, stored as `CARGO_REGISTRY_TOKEN` in `~/.zshenv` and `~/.profile`, both mode 600, outside the managed blocks. It stays. The trusted publisher for `soulmachine/purelymail`, workflow `release.yml`, was registered through the crates.io API with that token, so no browser login was needed.
**Decided-by:** human
**Justification:** crates.io's login runs through a GitHub popup the browser automation could not drive. Frank supplied the token on 2026-10-11 and asked for it to live in those two files.
**Outcome:** applied
**Ref:** (pending)
**Supersedes:** Q28 — the token part; the publish-then-register-then-tag sequence stands.

## Q30 — fleet/purelymail-cli — deviation

**Question:** How do the binary and the two tokens reach the other five personal hosts, and where do the tokens go on Linux?
**Options considered:** `cargo install` per host / a release-tarball role in each provisioning repo / tokens in each host's `.env` for the dotenv role / free-standing exports by hand; on Linux `~/.profile` as Frank said / `~/.bashrc` above the guard as the house doc says
**Chosen:** A `purelymail` role in macbook-provision, ubuntu-provision and omarchy-provision that installs the newest release tarball into `~/.local/bin`. Tokens as free-standing exports by hand: `~/.zshenv` on the four Macs, `~/.profile` on supermicro and mac-mini-2018-omarchy, all mode 600. On Omarchy, `~/.bash_profile` gained one line that sources `~/.profile`, because Arch's default never reads it and the tokens would have been dead there. Plain `ssh host 'cmd'` on the Linux hosts does not see them; `bash -lc` does, which is the fleet's remote-run form.
**Decided-by:** human
**Justification:** Frank named the files on 2026-10-11. The sourcing line is the smallest change that makes his choice hold on Arch. Company hosts get nothing, per fleet-ssh.md.
**Outcome:** applied
**Ref:** macbook-provision Q217, ubuntu-provision Q27, omarchy-provision Q12
