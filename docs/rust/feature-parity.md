# Feature parity

Status values: implementation (`done` / `partial` / `not started`),
verification (`PASS` = executed and passed, `FAIL`, `NOT_RUN`, `BLOCKED`).
"Differential" means the Rust and Go binaries produced identical stdout,
stderr, exit status (and `2>&1` order where checked) — see
`verification-report.md`.

| ID | Feature | Go source | Prio | Method | Contract / acceptance | Rust | Impl | Verified |
|---|---|---|---|---|---|---|---|---|
| F01 | Parse JSON, JSON Lines, concatenated docs; key order kept; duplicate keys (last value, first position); numbers verbatim | parser/scanner.go, json.go | P0 | Rust | same tree and output for any input | json/scanner.rs, json/mod.rs | done | PASS (unit, property, differential) |
| F02 | Syntax errors with line/byte column and Go wording | scanner.go | P0 | Rust | identical message | json/scanner.rs, unquote.rs, gostr.rs | done | PASS (unit, differential on corrupted input) |
| F03 | Nesting limit 10,000 | scanner.go | P0 | Rust | same limit; no crash at the limit | json/scanner.rs, main.rs | done | PASS (unit, binary test, differential at 9999/10000/10001) |
| F04 | Pretty print (2-space, Go escaping, no HTML escaping) | pipe/pretty.go, parser/encode.go | P0 | Rust | identical bytes | format/pretty.rs, json/encode.rs | done | PASS |
| F05 | `--compact` | pretty.go | P1 | Rust | identical bytes | format/pretty.rs | done | PASS |
| F06 | `-r` raw strings | pretty.go | P1 | Rust | identical bytes | format/pretty.rs | done | PASS |
| F07 | Colors, `--color auto/always/never`, `NO_COLOR` | pipe/formatter.go, cli/root.go | P1 | Rust | identical ANSI output and precedence | format/mod.rs, cli/mod.rs | done | PASS |
| F08 | `--sort-keys` | parser/json.go | P1 | Rust | byte-wise sort | json/mod.rs | done | PASS |
| F09 | Paths `-p` (syntax, errors, `[*]`, `.*`, `[-1]`) | parser/path.go | P0 | Rust | identical matches and errors | path.rs | done | PASS (unit, property, differential) |
| F10 | `-f` flat `path = value` | pipe/flat.go | P1 | Rust | identical bytes; paths resolve back | format/flat.rs | done | PASS |
| F11 | `-t` typed tree | pipe/typed.go | P2 | Rust | identical bytes | format/typed.rs | done | PASS |
| F12 | `-s` schema, merged across elements/documents | pipe/schema.go | P1 | Rust | identical bytes | format/schema.rs | done | PASS |
| F13 | `--diff` (files/stdin, `-p`, colors, summary, exit 1) | diff/diff.go, pipe/diff.go, cli/root.go | P1 | Rust | identical output and status | diff.rs, format/diff.rs, cli/mod.rs | done | PASS (differential incl. stream order); see D1 |
| F14 | CLI flags (pflag syntax, values, errors), help, usage, exit codes | cli/root.go (cobra/pflag) | P0 | Rust | identical bytes | cli/flags.rs, args.rs, help.rs, mod.rs | done | PASS (unit, integration, differential table of 67 command lines) |
| F15 | Input: file, stdin, `-`, terminal detection, file errors | cli/root.go | P0 | Rust | identical messages on Linux | cli/mod.rs | done | PASS (Linux locally; test suite also PASS on macOS and Windows in CI) |
| F16 | TUI navigation, expand/collapse, depth, paging | tui/model.go, update.go | P1 | Rust | same screens and cursor | tui/model.rs, view.rs | done | PASS (ported unit tests + pty comparison) |
| F17 | TUI search (text/path), `n`/`N`, highlight, Esc | tui/model.go, update.go, view.go | P1 | Rust | same matches and screens | tui/model.rs, view.rs, input.rs | done | PASS |
| F18 | TUI detail view, embedded JSON | tui/view.go | P2 | Rust | same content | tui/model.rs, view.rs, text.rs | done | PASS (unit, pty) |
| F19 | TUI copy `y`/`Y` (system clipboard, OSC 52 fallback) | tui/clipboard.go | P2 | Rust | same tool order and OSC 52 sequence | tui/clipboard.rs | done | PASS for logic and sequence; real clipboard BLOCKED (no display/clipboard tools in the container) |
| F20 | TUI type hints, help, light/dark theme | tui/update.go, styles.go, tokens.go | P2 | Rust | same text; same palette indices | tui/styles.rs, view.rs | done | PASS (text); colors NOT_RUN visually |
| F21 | TUI `-i -p` focus on matches | tui/model.go, cli/root.go | P1 | Rust | same initial cursor and match list | tui/model.rs | done | PASS |
| F22 | TUI with piped stdin (keys from the terminal) | bubbletea | P0 | Rust | works with `cat f | jv -i` | tui/mod.rs (crossterm) | done | PASS (pty test) |
| F23 | Shell completion: `jv completion <shell>`, `__complete` | cobra (implicit) | P2 | Rust + scripts reused verbatim | identical scripts and protocol | cli/completion.rs | done | PASS (differential, bash run); see D2 |
| F24 | Release artifacts for 6 targets | .github/workflows/release.yml | P1 | keep Go for now | – | – | not started | NOT_RUN |

## Known differences

| # | Difference | Why | Impact |
|---|---|---|---|
| D1 | `--diff` compares numbers exactly; Go treats literals equal when they agree to 64-bit binary precision (`1.00000000000000000001` vs `1`: Go "equal", Rust "changed") | Go behaviour is precision loss, not a design decision | only literals with >19 significant digits; reported as a change instead of silently equal |
| D2 | `JV_COMPLETION_DESCRIPTIONS=false` disables descriptions everywhere; in Go it only works under `jv __complete completion …` (cobra drops `__complete` from jv's root while completing, so it reads `__COMPLETE_COMPLETION_DESCRIPTIONS`) | follow cobra's documented behaviour, not the bug | only users setting that variable |
| D3 | Broken pipe: Rust exits with status 141 and no message; Go dies from SIGPIPE | Rust ignores SIGPIPE; 141 is what shells report for Go | `WIFSIGNALED` callers see an exit instead of a signal |
| D4 | The selected TUI line is highlighted across its full width | Go's highlight stops at the first colored span (lipgloss reset) | visual only |
| D5 | The Rust viewer does not query the terminal (OSC 11, cursor position) at startup | not needed for rendering | starts faster; no wait on terminals that ignore the queries |
| D6 | Word wrapping in the help/detail views is jv's own (break at spaces, hard-break long words, keep indentation), not lipgloss's | no lipgloss in Rust | line breaks of very long lines can differ; content is the same |
| D7 | Search input: `Ctrl+V` (paste from the clipboard) is not bound; terminal paste (bracketed paste) works | avoids reading the clipboard | use the terminal's paste |
| D8 | Windows clipboard via `clip.exe` (UTF-16) instead of the Win32 API | no unsafe/FFI | unverified on Windows; OSC 52 fallback |
| D9 | Non-ASCII error-message quoting: unassigned code points count as printable (Go escapes them) | no Unicode category tables | only exotic characters inside error messages |
| D10 | Search lowercasing uses Unicode full lowercase mapping (e.g. `İ` → `i̇`); Go maps rune to rune | std `char::to_lowercase` | a handful of characters |
| D11 | Flag *values* that are not valid UTF-8 are converted lossily (file names are kept as raw OS strings) | Rust strings | non-UTF-8 `-p`/`--theme` values only |
