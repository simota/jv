# Verification report

Statuses: PASS = executed and passed; FAIL; NOT_RUN; BLOCKED (environment).
Local environment: Linux x86_64, rustc/cargo 1.97.0 and 1.88.0, Go 1.24.7.
CI: `.github/workflows/rust.yml`, run #1 on GitHub Actions (ubuntu, macOS,
Windows, MSRV) — all jobs succeeded.

## Commands and results

| Check | Command (in `rust/`) | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | PASS |
| Lints | `cargo clippy --all-targets -- -D warnings` | PASS (0 warnings) |
| Docs | `RUSTDOCFLAGS=-D warnings cargo doc --no-deps` | PASS |
| Debug + release build | `cargo build`, `cargo build --release` | PASS |
| Unit tests | `cargo test --lib` | PASS (64) |
| CLI integration | `cargo test --test cli` | PASS (18) |
| Property tests | `JV_PROP_CASES=20000 cargo test --release --test robustness` | PASS (6 properties × 20,000 cases) |
| Differential vs Go | `JV_GO_BIN=… JV_DIFF_CASES=5000 cargo test --release --test differential` | PASS (7 suites: 25,000 generated cases + 113 fixed command lines + large/deep inputs) |
| MSRV | `rustup run 1.88 cargo test --locked` | PASS |
| Dependency audit | `cargo audit` (RustSec, 1,422 advisories, 90 crates) | PASS (no findings) |
| macOS / Windows tests | CI `platforms` jobs (`cargo test --locked`) | PASS (CI) |
| Go reference tests | `go test ./...` (repo root) | PASS (unchanged Go code) |
| TUI screens vs Go | `scripts/tui_smoke.py GO RUST` (pty, 70×20, piped stdin) | PASS (16/16 scenarios identical text; clean exit and screen restore) |
| Bash completion end to end | generated script sourced in bash, `__start_jv` invoked for 8 command lines, both builds | PASS (identical `COMPREPLY`) |

## What the differential tests cover

Each case runs both binaries with the same arguments, stdin and files and
compares stdout, stderr and exit status byte for byte; selected suites
also compare the interleaving of stdout and stderr (`2>&1`).

| Suite | Input |
|---|---|
| formatting | random documents (escapes, surrogates, invalid UTF-8 escapes, Unicode, U+2028, duplicate and odd keys, all number forms, whitespace, JSON Lines) × 9 output modes, with/without color |
| paths | real node paths of each document, with random suffixes and malformed syntax |
| syntax errors | valid documents corrupted by inserting/removing/truncating (control bytes, invalid UTF-8, NBSP, U+3000, quotes, backslashes…) |
| diff | document pairs (random or one valid edit apart), files and stdin, `-p`, color, merged streams |
| command line | 67 fixed command lines: help, every pflag error form, value parsing (base-0 ints, bools), files, directories, stdin, validation errors |
| large/deep | 3000-record array, 500-key object with duplicates, nesting 100/9999/10000/10001 |
| completion | 46 fixed command lines for `completion`/`__complete` + random completion requests |

The harness was itself checked by pointing it at a wrong binary
(`/bin/cat`), which fails every case.

## Compatibility findings

- Byte-identical on everything above, except the documented deviations
  D1–D11 in `feature-parity.md` (D1 and D2 were confirmed by running the
  Go binary; the others are outside what the tests exercise or are
  visual-only).
- Defect found and fixed in the port: `--diff` summary ordering when
  stdout and stderr share a terminal (regression tests in `tests/cli.rs`
  and the differential suite).

## Rust-specific safety

| Topic | Evidence |
|---|---|
| `unsafe` | none in jv's code (`grep -rn unsafe src` is empty) |
| Panics on input | property test feeds arbitrary bytes/paths; differential runs on corrupted input never exited with 101; remaining `expect`s guard internal invariants |
| Stack overflow | parser, diff, walk, flatten, typed formatter, layout are iterative; recursive formatters run on a 256 MiB thread; every format tested at nesting 10,000 via the binary |
| Integer overflow | index arithmetic uses `saturating_*`/`checked_*`/`rem_euclid`; `--depth` overflow handled like Go (`value out of range`); node count is `u32` (would need >4 billion nodes, i.e. hundreds of GB of RAM) |
| Resource release | terminal restored on quit and on error paths (`ratatui::restore`, panic hook from `ratatui::try_init`); child clipboard processes are waited for |
| Broken pipe | `jv big | head` exits quietly with 141 (test) |
| Concurrency | single-threaded apart from the worker thread; no shared state |

Not run: Miri and sanitizers (no nightly toolchain; with no `unsafe` code
their value would be limited to dependencies); fuzzing with cargo-fuzz
(needs nightly) — the property and differential generators cover the
same input space with deterministic seeds.

## Security review

- No network access, credentials, configuration files or persistent state.
- File access: only the paths given on the command line, read-only.
- Command execution: only fixed clipboard tools (`pbcopy`, `wl-copy`,
  `xclip`, `xsel`, `termux-clipboard-set`, `clip.exe`) without a shell;
  the text is passed on stdin, never as an argument.
- Terminal injection: pipe-mode output escapes control characters (JSON
  quoting) except with `-r`, which prints raw strings by design (same as
  Go). In the viewer, control characters in document text are shown as
  `\u00XX` escapes (`tui::text::display_safe`) and ratatui drops any left
  in a cell, so a string containing ESC sequences cannot drive the
  terminal (verified).
  The Go viewer used to pass raw strings to lipgloss in its detail view;
  it now shows control characters (C0 except newline and tab, DEL, C1) as
  `\u00XX` escapes everywhere it displays document text.
- Denial of service: nesting limited to 10,000; memory is proportional to
  input (no size limit, as in Go); hash maps keep SipHash.
- Dependencies: `cargo audit` clean; licenses permissive.

## Not verified

| Item | Status | Reason |
|---|---|---|
| Real clipboard tools (xclip, pbcopy, Windows) | BLOCKED | no display or clipboard in the container |
| TUI on macOS/Windows terminals | NOT_RUN | no interactive terminals available; unit tests pass there in CI |
| Colors as rendered (theme palettes) | NOT_RUN | pty comparison checks text only; styles are unit-tested |
| zsh/fish/powershell completion end to end | NOT_RUN | shells not installed; scripts are byte-identical to Go's and the protocol is differential-tested |
| Cross-compiled release artifacts (arm64, etc.) | NOT_RUN | no release workflow for Rust yet |
