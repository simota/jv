# Repository analysis (before the port)

Source of truth: the code, tests and workflows at commit `066ecb1` (`main`).

## What jv is

A terminal JSON viewer distributed as a single static binary (`go install`
or GitHub Release archives). Two modes:

- **Pipe mode** (default): reads JSON / JSON Lines / concatenated documents
  from a file or stdin and prints them pretty, compact, as `path = value`
  lines (`-f`), as a typed tree (`-t`), as an inferred schema (`-s`), or a
  structural diff of two inputs (`--diff`). `-p` selects values by path.
- **Interactive mode** (`-i`): a collapsible tree viewer (bubbletea) with
  search, path jumps, a full-value view and clipboard copy.

## Technology stack

| Area | Go implementation |
|---|---|
| Language | Go 1.24 (`go.mod`: `go 1.24.1`), `CGO_ENABLED=0` |
| CLI | spf13/cobra 1.10.2 + pflag 1.0.9 |
| TUI | charmbracelet/bubbletea 1.3.10, bubbles 0.21 (textinput, viewport), lipgloss 1.1, x/ansi |
| Clipboard | atotto/clipboard (xclip/xsel/wl-copy/pbcopy/...), aymanbagabas/go-osc52 fallback |
| Terminal detection | golang.org/x/term |
| JSON | hand-written scanner (`internal/parser/scanner.go`); `encoding/json` only for string unquoting and encoding |
| Build/CI | `Makefile`; `.github/workflows/release.yml` builds linux/darwin/windows × amd64/arm64 on release |

There is no database, network access, authentication, configuration file,
concurrency (beyond bubbletea's event loop), or persistent state.

## Modules (≈3,000 lines of Go, ≈1,000 of tests)

| Package | Files | Responsibility | Depends on |
|---|---|---|---|
| `cmd/jv` | main.go | entry point | cli |
| `internal/parser` | scanner.go, json.go, encode.go, path.go | order-preserving parser (max nesting 10000, duplicate keys: last value / first position, verbatim numbers), syntax errors with line/column, `Node` tree with parent links, path syntax and queries, JSON encoding | encoding/json |
| `internal/pipe` | pretty.go, flat.go, typed.go, schema.go, diff.go, formatter.go | pipe-mode formatters and ANSI colors | parser, diff |
| `internal/diff` | diff.go | structural diff (`+ - ~` by path), numeric equality via `big.Float` | parser |
| `internal/cli` | root.go | cobra command, flag validation, input loading, mode dispatch, exit codes | all |
| `internal/tui` | model.go, update.go, view.go, styles.go, tokens.go, clipboard.go, run.go | viewer state, key handling, rendering, clipboard | parser, bubbletea, lipgloss |

## External contracts (must be preserved)

- Command line: flag names/shorthands/defaults, pflag syntax, cobra help and
  usage text, error messages (`Error: ...` on stderr, usage on stdout for
  flag errors), exit codes (0, 1; `--diff` exits 1 when inputs differ).
- Output bytes of every pipe-mode format, with and without color
  (`--color` defaults to `always`; `NO_COLOR` applies only when `--color`
  is not given).
- Syntax error texts, including Go-style character quoting and byte columns.
- Path syntax (`$.a[0]`, `["key"]`, `[*]`, `.*`, `[-1]`, `.[0]`) and its errors.
- TUI key bindings and screens; clipboard via system tool, then OSC 52.
- Shell completion that cobra adds implicitly (`jv completion <shell>`,
  `jv __complete`), although the README does not mention it.

## Risks identified for a port

1. Exact error-message compatibility depends on Go library behaviour
   (`strconv.Quote`, `encoding/json` quoting, `strconv.ParseInt` base 0,
   errno texts, pflag/cobra wording and ordering).
2. Deep recursion: Go stacks grow; Rust threads have fixed stacks.
3. Invalid UTF-8 inside JSON strings is accepted by Go (replaced per byte);
   Rust strings must stay valid UTF-8.
4. TUI libraries differ (bubbletea vs. ratatui), so screens must be compared
   in a terminal, not just at the model level.
5. Numbers are kept as literal text; the diff's numeric equality uses
   64-bit-mantissa `big.Float`.

## Classification

| Category | Code |
|---|---|
| Must be ported (the product) | parser, path, diff, formatters, CLI, TUI |
| Kept in Go | the whole Go implementation, as the reference for differential testing and as the released binary until switch-over; the Go release workflow |
| Reused as-is | `testdata/sample.json`; cobra-generated completion scripts (embedded verbatim) |
| Technical debt noted (not changed in Go) | see "Suspected bugs" in `baseline-report.md` |
