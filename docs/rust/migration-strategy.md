# Migration strategy

## Options considered

| | A: full Rust port | B: gradual, module by module | C: partial (hot paths via FFI) |
|---|---|---|---|
| Fit for jv | single binary, no external services, ~3k LOC | modules are internal packages of one binary; mixing would need cgo/FFI inside the binary | the "hot path" is the whole program (parse → format) |
| Coexistence | both binaries side by side; the Go one is the oracle | Go + Rust in one binary | Go + Rust in one binary |
| Build/distribution | one `cargo build`; static binary per target | cgo breaks `CGO_ENABLED=0` releases (glibc dependency, see `docs/RELEASING.md`) | same cgo problem |
| Risk | contained: switch at release time, roll back by releasing the Go build | many small cut-overs with FFI boundaries | FFI cost per value, two toolchains |
| Verification | black-box differential tests on the binaries | per-module tests across FFI | per-module tests across FFI |

**Chosen: A (full port), delivered side by side.** jv is a small,
self-contained CLI whose entire contract is visible at the process
boundary, which makes a black-box differential comparison against the Go
binary the strongest possible check. B and C would require cgo, which the
project explicitly avoids (`CGO_ENABLED=0`), and would buy nothing: there
is no independently deployable part.

"Gradual" still applies at the *release* level: the Rust binary lives in
`rust/`, is verified continuously against the Go build in CI, and replaces
the Go release only when the maintainer decides (see `migration-report.md`).

## Scope

- Ported: everything user-visible — parser, paths, diff, all formats, CLI
  (including cobra's implicit shell completion), the TUI and clipboard.
- Not ported: nothing. The Go code is kept unchanged as the reference
  implementation and remains the released artifact for now.

## Principles applied

- Byte-for-byte compatibility of outputs, error texts and exit codes is
  the default; any deviation is listed in `feature-parity.md` with a reason.
- Suspected Go bugs are not copied blindly (numeric precision, completion
  env var); harmless quirks that scripts could depend on are kept.
- No new runtime dependencies beyond what the TUI needs (ratatui/crossterm,
  unicode-width); no `unsafe`.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Hidden behavioural differences | differential tests (~25k generated cases per run locally, 2k per generator in CI) + pty screen comparison |
| Stack overflow on deep input | iterative parser/diff/flatten/walk; binary runs on a 256 MiB (virtual) stack thread for the recursive formatters; tested at max nesting |
| Terminal differences between bubbletea and ratatui | pty-driven screen comparison of 16 scenarios |
| Toolchain drift | MSRV 1.88 pinned in `Cargo.toml` and tested in CI; `Cargo.lock` committed |
