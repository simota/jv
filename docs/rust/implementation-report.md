# Implementation report

## Delivered

A complete Rust implementation of jv in `rust/` (library + `jv` binary),
covering every feature in `feature-parity.md` (F01–F23). The Go
implementation is unchanged.

## Files

| Path | Content |
|---|---|
| `rust/Cargo.toml`, `Cargo.lock`, `.gitignore` | crate definition (edition 2024, MSRV 1.88), locked dependencies |
| `rust/src/main.rs` | entry point (large-stack worker thread) |
| `rust/src/gostr.rs` | Go `strconv`/`utf8` compatibility |
| `rust/src/json/` | document model, parser, string decoding, encoder (+ tests) |
| `rust/src/path.rs`, `diff.rs` | path queries, structural diff (+ tests) |
| `rust/src/format/` | pretty/compact/raw, flat, typed, schema, diff formatters (+ tests) |
| `rust/src/cli/` | CLI, pflag-compatible flags, cobra texts, shell completion (+ scripts) |
| `rust/src/tui/` | viewer model, view, input, styles, text helpers, clipboard, terminal loop (+ tests) |
| `rust/tests/cli.rs` | CLI integration tests (port of `root_test.go`) and binary-level tests |
| `rust/tests/differential.rs` | Rust-vs-Go differential tests (needs `JV_GO_BIN`) |
| `rust/tests/robustness.rs`, `tests/common/` | property tests and shared generators |
| `rust/examples/gen_bench_data.rs`, `scripts/bench.py` | benchmark input and runner |
| `rust/scripts/tui_smoke.py` | drives viewers in a pty and compares screens |
| `.github/workflows/rust.yml` | CI for the Rust port |
| `Makefile` | added `rust-build`, `rust-test`, `rust-difftest` (existing targets unchanged) |
| `docs/rust/*.md` | these documents |
| `README.md` | section on building/testing the Rust version |

## Crates

`ratatui` 0.30 (crossterm backend only), `unicode-width` 0.2. No
dev-dependencies; generators, temp dirs and base64 are local code.

## Reuse of existing assets

- `testdata/sample.json` is used by the Rust tests.
- The Go tests were ported test by test (parser, path, pipe, diff, cli,
  tui), with the same inputs and expectations.
- cobra's generated completion scripts are embedded verbatim
  (`rust/src/cli/completion/*.txt`); `--no-descriptions` swaps the single
  `__complete` invocation for `__completeNoDesc`, exactly as cobra does.
- Help and usage texts were captured from the Go binary.

## Not implemented

Nothing from the feature list. Differences are listed in
`feature-parity.md` (D1–D11). A release workflow for Rust binaries was
intentionally not added (switch-over needs the maintainer's decision).

## Technical notes

- The Go TUI stores expand state in the nodes; the Rust model keeps it in
  `Model::expanded`, so the parsed document stays immutable while viewing.
- `diff::Change` is an enum (`Added`/`Removed`/`Changed`) instead of a
  struct with optional old/new values, so invalid combinations cannot exist.
- Two bugs were found in the port during verification and fixed:
  1. the `--diff` summary (stderr) was printed before the buffered changes
     (stdout) when both went to one terminal; found by the merged-stream
     differential check, fixed by flushing first;
  2. a test generator produced invalid JSON by joining two numbers; the
     parser was correct, the generator was fixed.
