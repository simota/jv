# Implementation plan (as executed)

| # | Task | Depends on | Done when | Tests | Status |
|---|---|---|---|---|---|
| 1 | Go-compatible quoting helpers (`gostr`) | – | `strconv.Quote`/`QuoteRune` and per-byte UTF-8 replacement match Go | unit | done |
| 2 | Document arena, parser, unquote, encoder | 1 | all Go parser tests ported and passing; error texts match | unit + differential (formats, errors) | done |
| 3 | Path compile/find | 2 | Go path tests pass; errors match | unit + differential (paths) + property (every path resolves) | done |
| 4 | Diff | 2 | Go diff tests pass | unit + differential (diff) + property (symmetry) | done |
| 5 | Formatters (pretty, compact, raw, flat, typed, schema, diff) | 2–4 | Go pipe tests pass | unit + differential | done |
| 6 | pflag-compatible flags, cobra texts, CLI dispatch | 5 | Go CLI tests ported; usage/errors match | integration + differential (CLI table) | done |
| 7 | TUI model, view, input, clipboard, terminal loop | 2, 3 | Go TUI tests ported; pty screens match Go | unit + `scripts/tui_smoke.py` | done |
| 8 | Shell completion (cobra `completion`, `__complete`) | 6 | scripts and protocol output match | integration + differential + bash script run | done |
| 9 | Robustness: deep nesting, broken pipe, arbitrary bytes | 2–6 | no panics/overflows | integration + property | done |
| 10 | Performance: profile, remove per-string allocations | 2 | faster than Go on all benchmark cases | `scripts/bench.py` | done |
| 11 | CI, Makefile targets, MSRV check | all | workflow runs fmt/clippy/doc/test/differential/MSRV/macOS/Windows | CI | done (see verification report for CI result) |
| 12 | Release workflow for Rust binaries, switch-over | 11 + maintainer decision | – | – | **not started** (needs approval; see migration report) |
