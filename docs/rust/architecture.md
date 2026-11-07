# Rust architecture

## Toolchain and targets

- Edition 2024, MSRV **1.88** (`rust-version` in `rust/Cargo.toml`; needed
  by ratatui 0.30 and let-chains). Verified locally with 1.88.0 and 1.97.0.
- One crate, `rust/` (library `jv` + binary `jv`); a workspace would only
  add ceremony at this size. `publish = false`.
- Release profile: thin LTO, `codegen-units = 1`, stripped.
- Targets: whatever the Go release ships (linux/darwin/windows ×
  amd64/arm64). Pure Rust dependencies, no C toolchain needed.
- No feature flags.

## Dependencies

| Crate | Why | License |
|---|---|---|
| `ratatui` 0.30 (feature `crossterm` only) | terminal rendering, alternate screen, panic-safe restore | MIT |
| (via ratatui) `crossterm` 0.29 | raw mode, key/resize/paste events; reads keys from `/dev/tty` when stdin is piped | MIT |
| `unicode-width` 0.2 | cell widths for truncation/wrapping | MIT/Apache-2.0 |

Deliberately not used: serde/serde_json (cannot keep literal numbers,
Go-compatible error texts and per-byte UTF-8 replacement), clap (different
syntax and messages from pflag), arboard (X11 clipboards are lost when the
process exits; command-line tools like the Go version use survive), a
base64 crate (20 lines). All transitive crates (including platform-specific ones) are MIT, Apache-2.0, Zlib, BSL-1.0 or Unicode-3.0 licensed.

## Modules

```
src/
  main.rs          worker thread with a large stack -> cli::main
  lib.rs
  gostr.rs         Go strconv.Quote / QuoteRune, utf8.DecodeRune, per-byte U+FFFD
  json/
    mod.rs         Document arena: Node, NodeId, Key, Value, Str (string pool), paths
    scanner.rs     iterative parser, Go-compatible errors, duplicate-key rule
    unquote.rs     JSON string literal decoding with encoding/json semantics
    encode.rs      quote_string / marshal (Go encoder escaping, no HTML escaping)
  path.rs          Query::compile / find (syntax and errors of path.go)
  diff.rs          compare -> Vec<Change> (enum), numbers_equal, summary
  format/          Formatter trait; pretty/compact/raw, flat, typed, schema, diff
  cli/
    mod.rs         execute(args, Env) -> exit code; I/O injected via Env
    flags.rs       generic pflag-compatible parser (syntax, ParseBool/ParseInt, messages)
    args.rs        root flag table -> Options
    completion.rs  cobra's completion command + __complete protocol (+ embedded scripts)
    help.rs        cobra help/usage texts, verbatim
  tui/
    mod.rs         run(): terminal setup, event loop, key mapping
    model.rs       Model state + update(Msg) -> Command (no terminal needed)
    view.rs        Model -> Vec<Line> (render() and plain-text view())
    input.rs       single-line editor (bubbles textinput bindings)
    text.rs        width-aware truncate / wrap
    styles.rs      theme/color -> ratatui styles
    clipboard.rs   pbcopy / wl-copy / xclip / xsel / termux / clip.exe, then OSC 52
```

## Data model and ownership

`Document` owns everything: `nodes: Vec<Node>`, `roots: Vec<NodeId>` and a
`strings: String` pool holding every key, string value and number literal.
`Node { key: Key, value: Value, children: Vec<NodeId>, parent: Option<NodeId>, depth }`.

- `NodeId` (u32 index) replaces Go's `*Node`. Parent links are plain ids,
  so there are no reference cycles, `Rc` or `RefCell`.
- `Key` is `Root | Index(usize) | Name(Str)`; `Value` is
  `Object | Array | String(Str) | Number(Str) | Bool | Null`. Both are `Copy`;
  text is read with `Document::text`, `key_text`, `string`.
- Formatters, the diff and path queries borrow `&Document`. Only
  `sort_keys` and `wrap_documents` mutate it, before anything else reads it.
- The TUI owns the `Document`. UI state (expanded flags, matches) lives in
  the `Model` (`expanded: Vec<bool>` indexed by `NodeId`), not in the
  nodes as in Go.

The string pool and exactly-sized child vectors (children are collected on
a scratch stack and moved out when a container closes) cut instructions
by 35% and peak memory by 23% (623 → 481 MiB on the 55 MB benchmark) compared with one allocation per string.

## Errors

- `json::ParseError { Syntax(SyntaxError{msg, offset, line, column}), Empty }`
- `path::PathError(String)`, carrying Go's exact message.
- The CLI maps everything to a private `Failure` enum:
  `Usage(msg)` / `UsageOf(usage, msg)` (usage on stdout + `Error:`),
  `Error(msg)`, `Exit(code)` (silent: `--diff` with changes, broken pipe).
- OS errors are rendered like Go's `syscall.Errno` (`open x: no such file
  or directory`).
- No `unsafe`. The remaining `expect`/`unreachable!` calls guard internal
  invariants (validated escapes, ASCII literals, flag kinds), not input.

## Recursion and stack

Untrusted input never drives unbounded recursion in the parser, the diff,
`walk`, `flatten`, the typed formatter, `sort_keys` or the TUI layout:
they use explicit stacks. `marshal`, the pretty and schema formatters
recurse once per level; nesting is capped at 10,000 by the parser, and
`main` runs on a thread with a 256 MiB stack (virtual reservation).
`rust/tests/cli.rs` exercises every format at maximum nesting.

## Concurrency

None needed: jv reads all input, then formats it. The TUI is a
synchronous loop (`event::read` → `Model::update` → `draw`); a clipboard
copy is a short child process. No async runtime, no locks.

## TUI

`Model` is a pure state machine: `update(Msg::Key|Resize|Paste) -> Command`.
`view_lines()` produces styled lines; `render()` writes them into the
ratatui buffer (no widgets), and `view()` returns plain text for tests.
Rendering only touches the visible rows, so large documents stay
responsive. Document text (keys, values, paths, the detail view) goes through
`text::display_safe`, which shows control characters as `\u00XX` escapes
(as the Go viewer does), so string contents cannot inject escape sequences
into the terminal; ratatui also drops any control character left in a cell.
Tabs in the detail view are expanded to four spaces, as lipgloss does.

## Compatibility layer

`gostr.rs`, `json/unquote.rs`, `cli/flags.rs` and the verbatim texts in
`cli/help.rs` and `cli/completion/*.txt` exist only to reproduce Go library
behaviour byte for byte. They are isolated so they can be relaxed later if
exact compatibility stops mattering.
