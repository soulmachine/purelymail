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
