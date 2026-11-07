# Performance report: Go implementation

This covers the optimization of the released Go implementation
(`cmd/`, `internal/`). The Rust port in `rust/` was not changed (see
`docs/rust/performance-report.md`). Every number below was measured.

## Setup

| | |
|---|---|
| Machine | cloud VM, Linux 6.18 x86_64, 4 vCPU, shared host (a few % noise between identical runs; one outlier of +21% was seen) |
| Build | Go 1.24.7, `CGO_ENABLED=0 go build ./cmd/jv` (as the release workflow) |
| Input | `cargo run --release --example gen_bench_data -- DIR` (in `rust/`): `records.json` (250,000 records, 55 MB), `records.jsonl`, `wide.json` (200,000 keys); `records2.json` is a copy with one changed value |
| Method | `scripts/bench_ab.py BEFORE AFTER DIR 7`: one warm-up each, then 7 runs of the two binaries interleaved; median and max wall time, median CPU time (user+sys), peak RSS (`ru_maxrss`); output to `/dev/null` |

## End-to-end results (55 MB input)

| case | wall before (s) | wall after (s) | wall | max before → after | CPU | RSS before → after (MiB) | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| pretty | 2.912 | 1.520 | −48% | 3.302 → 1.640 | −50% | 1133 → 538 | −53% |
| pretty+color | 3.934 | 1.308 | −67% | 4.077 → 1.439 | −70% | 1562 → 537 | −66% |
| compact | 2.028 | 1.181 | −42% | 2.163 → 1.222 | −43% | 945 → 537 | −43% |
| sort-keys | 2.893 | 1.303 | −55% | 3.015 → 1.405 | −62% | 1164 → 537 | −54% |
| flat (`-f`) | 3.567 | 1.192 | −67% | 4.177 → 1.229 | −70% | 1571 → 537 | −66% |
| typed (`-t`) | 3.647 | 1.270 | −65% | 3.889 → 1.323 | −69% | 1702 → 537 | −69% |
| schema (`-s`) | 1.362 | 1.177 | −14% | 1.776 → 1.323 | −28% | 636 → 538 | −16% |
| path `[*].address.city` | 1.470 | 1.142 | −22% | 1.523 → 1.190 | −32% | 680 → 562 | −17% |
| JSON Lines compact | 2.181 | 1.251 | −43% | 2.382 → 1.521 | −45% | 1001 → 529 | −47% |
| wide object | 0.208 | 0.130 | −38% | 0.219 → 0.160 | −43% | 76 → 47 | −38% |
| diff (two 55 MB files) | 4.346 | 2.240 | −49% | 4.786 → 2.301 | −45% | 1441 → 1030 | −29% |
| start-up (7-byte input) | 0.002 | 0.002 | equal | | equal | 14 → 14 | equal |

Other workloads (medians; p95/p99 from 100–400 interleaved runs):

| case | before | after |
|---|---:|---:|
| start-up, 400 runs | p50 2.19 ms, p95 2.96 ms, p99 3.78 ms | p50 2.21 ms, p95 3.03 ms, p99 3.65 ms (equal within noise) |
| tiny input through a pipe, 400 runs | p50 2.36, p95 3.76, p99 5.03 ms | p50 2.35, p95 3.29, p99 4.31 ms |
| 8.5 KB input, 300 runs | p50 2.98, p95 4.70 ms | p50 2.90, p95 4.38 ms |
| 1 MB input, pretty | p50 61.5, p95 82.6 ms, 27 MB | p50 30.5, p95 41.8 ms, 16 MB |
| 1 MB input, `-f` | p50 83.0, p95 107 ms, 36 MB | p50 28.6, p95 38.5 ms, 16 MB |
| 55 MB from a pipe, `--compact` | 2.72 s, 933 MB | 1.24 s, 510 MB |
| 55 MB redirected stdin, `-p [0]` | 1.60 s, 634 MB | 0.96 s, 510 MB |

Memory kept alive after parsing (what the TUI holds; `runtime.GC()` then
`HeapAlloc`, 250,000 records):

| input | before | after |
|---|---:|---:|
| compact, 55 MB | 513 MB | 437 MB |
| 2-space indent, 88 MB | 513 MB | 437 MB |
| 8-space indent, 164 MB | 513 MB | 437 MB |

Binary size: 5,976,794 → 5,983,852 bytes (+0.1%).

## In-process benchmarks (20,000 records; `go test -bench`)

| benchmark | before ns/op | after ns/op | before allocs | after allocs |
|---|---:|---:|---:|---:|
| Parse | ~87 M | ~58 M | 1,364,802 | 378,935 |
| Pretty (`Format`) | ~72 M | ~31 M | 580,017 | 12 |
| PrettyColor | ~115 M | ~52 M | 840,020 | 13 |
| Compact | ~70 M | ~20 M | 580,015 | 11 |
| Flat | ~127 M | ~31 M | 1,040,083 | 12 |
| Typed | ~138 M | ~56 M | 1,620,043 | 14 |
| Schema | ~7.6 M | ~8.0 M | 76 | 76 (unchanged code; equal within noise) |
| SortKeys | ~12.0 M | ~6.5 M | | |
| TUI search, 50,000 records | ~28 M | ~17 M | 250,002 | 50,012 |
| TUI layout, fully expanded | ~62 M | ~13 M | 101 (66.8 MB) | 3 (12.8 MB) |

## What was slow, and the changes

Profiling showed the parser spending most of its time in `malloc` and the
garbage collector, and the formatters building a string per token and then
holding the whole output (twice) before writing it.

1. **Parser allocations** (`internal/parser/scanner.go`): nodes come from
   chunks; children are collected on a shared stack and copied into an
   exactly sized slice; keys, strings and numbers are copied into shared
   string chunks (the input is not kept alive); array index keys are shared;
   escaped strings are decoded by a port of encoding/json's decoder
   (`quote.go`), falling back to `json.Unmarshal` only to produce the error
   message for invalid literals.
2. **Smaller nodes** (`json.go`): `NodeType` is a `uint8` with a `String`
   method, `Depth` is `int32`; `Node` went from 96 to 72 bytes.
3. **Streaming output** (`internal/pipe/writer.go`): formatters append
   straight into one buffer that is written out every 64 KiB, so output is
   never held whole. `Format` still returns the same string.
4. **No temporary strings in formatting**: `AppendQuoted` (same bytes as
   encoding/json without HTML escaping) replaces creating a `json.Encoder`
   per non-ASCII string; numbers skip `fmt.Sprintf`; flat paths and typed
   prefixes are built in reusable buffers; the simple-key regexp became a
   loop.
5. **Diff** builds a change's path only when it records the change, matches
   keys of small objects without maps, and compares strings directly.
6. **Input reading** reads straight into a string (no second copy), uses the
   file size for redirected stdin, and joins pipe input once.
7. **`--sort-keys`** uses `slices.SortStableFunc` (same stable order).
8. **TUI**: `layout` counts rows first and allocates once; search compares
   ASCII text without lower-casing a copy (same result as
   `strings.ToLower`).

## Verification

- `go vet`, `gofmt`, `go test ./...` and `go test -race ./...` pass.
- Output identity: about 1,500 command lines (28 edge-case files × 19 flag
  sets, file and stdin, diffs between files, large files, pipes) produced
  byte-identical stdout, stderr and exit codes before and after.
- The Rust differential suite (`JV_DIFF_CASES=3000`) passes against the new
  Go binary.
- New tests: `AppendQuoted`/`QuoteString` against encoding/json for every
  byte and BMP code point and by fuzzing; the string decoder against
  `json.Unmarshal` by fuzzing; the simple-key loop against the old regexp;
  streaming in bounded chunks and keeping the first write error; diff with
  small and large objects and duplicate keys; `containsFold`; reading input
  around chunk sizes and with read errors. The new diff and quoting tests
  also pass on the original code.
- Fuzzing (60 s each, after the final change): parser vs encoding/json,
  quoting, unquoting.

## Not done / limits

- One heap allocation per string or number value remains: storing a string
  in `Node.Value` (`any`) allocates, and changing the field's type would
  touch every user of the tree.
- GC still takes a large share of parse time; raising `GOGC` would trade
  memory for speed, so it was not changed.
- The interactive TUI was measured through benchmarks only (search and
  layout), not by driving a terminal.
- Single machine; numbers on other hardware will differ.
