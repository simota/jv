# Performance report

All numbers were measured; nothing is extrapolated.

## Setup

| | |
|---|---|
| Machine | cloud VM, Linux 6.18 x86_64, 4 vCPU Intel Xeon @ 2.10 GHz, 15 GiB RAM (shared host: expect a few % noise) |
| Go build | Go 1.24.7, `CGO_ENABLED=0 go build ./cmd/jv` (as the release workflow) |
| Rust build | rustc 1.97.0, `cargo build --release` (thin LTO, 1 codegen unit, stripped) |
| Input | `cargo run --release --example gen_bench_data -- DIR`: 250,000 API-like records (`records.json`, 55 MB array; `records.jsonl`, JSON Lines); `wide.json` (one object, 200,000 keys, 3.6 MB); diff compares `records.json` with a copy that has one changed digit |
| Method | `rust/scripts/bench.py GO RUST DIR 5`: one warm-up run, then the median wall time of 5 runs; peak RSS is the child's `ru_maxrss`; output to `/dev/null` |

## Results (final build)

| case | Go time (s) | Rust time (s) | speedup | Go RSS (MiB) | Rust RSS (MiB) |
|---|---:|---:|---:|---:|---:|
| pretty | 2.091 | 0.862 | 2.43x | 1134 | 513 |
| pretty+color | 3.153 | 1.016 | 3.10x | 1563 | 565 |
| compact | 1.620 | 0.771 | 2.10x | 945 | 481 |
| sort-keys | 2.362 | 1.012 | 2.33x | 1159 | 514 |
| flat | 2.592 | 1.487 | 1.74x | 1530 | 758 |
| typed | 2.855 | 1.124 | 2.54x | 1666 | 563 |
| schema | 1.173 | 0.679 | 1.73x | 636 | 481 |
| path [*].address.city | 1.200 | 0.612 | 1.96x | 680 | 481 |
| jsonl compact | 1.750 | 0.825 | 2.12x | 999 | 480 |
| wide object | 0.152 | 0.093 | 1.63x | 75 | 39 |
| diff | 1.029 | 0.582 | 1.77x | 686 | 481 |
| startup (tiny input) | 0.002 | 0.002 | 1.19x | 14 | 14 |

Binary size: Go 6.1 MB, Rust 0.9 MB.

Rust is faster in every case (1.6x–3.1x on real work) and uses 24–64%
less peak memory. Start-up is ~2 ms for both. No case regressed.

## Optimization done

Profiling with callgrind (`-s` on 3 MB of JSON Lines) showed ~42% of
instructions in `malloc`/`free`: one allocation per key and string, an
extra clone of every object key for duplicate detection, and growing
`Vec`s for children.

| Step | Change | Instructions (callgrind) |
|---|---|---:|
| initial port | | 385 M |
| 1 | compare keys by reference; collect children on a shared scratch stack and move them into an exactly-sized `Vec` when the container closes | 355 M (−8%) |
| 2 | store all keys, strings and number literals in one pooled buffer per document (`Str` handles) | 250 M (−35% total) |

Wall time for "pretty" went from 1.22 s to 0.86 s and peak memory from
655 MiB to 513 MiB between the first port and the final build. All
functional and differential tests were re-run after each step.

## Not optimized (and why)

- `flat` is the slowest Rust case: it builds a path string per leaf, like
  Go. Acceptable; not worth extra complexity.
- SipHash is used for the schema's key index and for objects with more
  than 16 members. A faster non-keyed hash would be open to collision
  attacks with crafted input.
- Parsing could skip per-string UTF-8 validation with `unsafe`; ~5% of
  instructions, not worth giving up the no-`unsafe` property.
- Freeing the document at exit costs a little; leaking it deliberately
  was rejected to keep the library usable in-process.

## Limits of the measurement

Single machine, single run of the suite; no CPU pinning. The TUI was not
benchmarked (it renders only visible rows in both versions).
