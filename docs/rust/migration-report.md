# Migration report

## Scope completed

Every feature of jv is implemented in Rust (`rust/`), F01–F23 in
`feature-parity.md`. The remaining item, F24 (release artifacts), is the
switch-over itself and deliberately left to the maintainer.

## What remains in Go

The whole Go implementation, unchanged, for two reasons:

1. It is the **released** product (`go install`, GitHub Releases built by
   `release.yml`). Replacing it is an outward-facing decision.
2. It is the **oracle** for the differential tests that guard
   compatibility. Keep it until the Rust version is released and the
   remaining deviations are accepted.

## Switch-over plan (proposal)

1. **Review** the deviations D1–D11 (`feature-parity.md`) and accept or
   request changes (D1 and D2 are behaviour changes; the rest are minor).
2. **Pre-release**: add a release job that builds the Rust binary for the
   same six targets (`x86_64`/`aarch64` × linux-musl or gnu, apple-darwin,
   pc-windows-msvc) with the same archive names
   (`jv_${VERSION}_${os}_${arch}.tar.gz` / `.zip`, binary inside named like
   today), and publish it as a pre-release (e.g. `v0.2.0-rc.1`). Users
   can compare both. Static Linux builds: use the `*-musl` targets to keep
   the "no glibc dependency" property of `CGO_ENABLED=0`.
3. **Release**: switch `release.yml` from `go build` to the Rust job.
   Note that `go install github.com/simota/jv/cmd/jv@latest` keeps
   installing the Go version as long as the Go code stays on `main`;
   document `cargo install --git https://github.com/simota/jv jv` (or the
   release archives) as the installation method.
4. **Retire Go** only after a release cycle without regressions and with
   the maintainer's explicit approval: then move the differential suite to
   a fixed corpus of recorded Go outputs, and delete `cmd/`, `internal/`,
   `go.mod`, `go.sum`.

There is no data to migrate and no running service: jv is a stateless CLI.
Parallel operation is trivial (both binaries can be installed side by side
under different names).

## Rollback

Until step 4 the Go code remains on `main`, so rolling back is:
re-run `release.yml` from the Go-based revision (or re-publish the previous
release's assets). Nothing persists between runs, so there is nothing to
undo for users beyond reinstalling.

## Monitoring after release

Watch for issues about: output differences in scripts (the differential
suite should prevent them), terminal compatibility of the viewer (different
TUI library), clipboard behaviour on Windows (D8, unverified), and
completion scripts.

## CI/CD

`.github/workflows/rust.yml` (new; `release.yml` untouched):

| Job | Runs |
|---|---|
| lint, test, differential (linux) | `go test ./...`, builds the Go reference binary, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo doc -D warnings`, `cargo test --release` with the differential (2,000 cases/suite) and property (10,000 cases) tests, release build |
| MSRV (1.88) | `cargo test --locked` |
| test (macos-latest, windows-latest) | `cargo test --locked` (differential tests skip without Go) |
| dependency audit | `cargo audit` |

Local equivalents: `make rust-test`, `make rust-difftest`, `make rust-build`.

## Final quality gate

| Area | Criterion | Result |
|---|---|---|
| Function | all P0 acceptance criteria | PASS |
| Compatibility | required external contracts kept | PASS, with 11 documented deviations (2 behavioural, by choice) |
| Build | target configurations build | PASS (Linux, macOS, Windows in CI; MSRV) |
| Tests | required tests pass | PASS (unit, integration, property, differential, pty) |
| Rust quality | fmt, clippy, docs | PASS |
| Safety | no known unresolved serious issue | PASS (no `unsafe`, deep input and broken pipe tested) |
| Error handling | important failure cases | PASS (all error paths differential-tested) |
| Concurrency | – | not applicable (single-threaded design) |
| Performance | no regression vs Go | PASS (1.6x–3.1x faster, 24–64% less memory) |
| Regression | existing system unaffected | PASS (Go code untouched; `go test ./...` and `make build/test` pass) |
| Migration | switch and rollback steps clear | PASS (plan above; not executed) |
| Documentation | build, use and verification reproducible | PASS |

- **Implementation completeness: COMPLETE** for the agreed scope (all
  features ported; release automation intentionally pending).
- **Verification: CONDITIONAL PASS** — everything executable here passed;
  real clipboard tools, interactive terminals on macOS/Windows and
  non-bash shells were not exercised.
- **Migration readiness: CONDITIONAL** — ready for a pre-release; needs the
  maintainer to accept D1–D11, add the Rust release job, and do a manual
  check of the viewer and clipboard on macOS and Windows.
