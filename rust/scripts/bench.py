#!/usr/bin/env python3
"""Compare the Go and Rust jv binaries: median wall time and peak RSS.

usage: bench.py GO_BIN RUST_BIN DATA_DIR [RUNS]

DATA_DIR comes from `cargo run --release --example gen_bench_data -- DATA_DIR`.
Each command writes to /dev/null; peak RSS is the child's ru_maxrss.
"""
import os
import statistics
import subprocess
import sys
import time


def measure(cmd, cwd):
    devnull = os.open(os.devnull, os.O_WRONLY)
    start = time.perf_counter()
    proc = subprocess.Popen(cmd, cwd=cwd, stdout=devnull, stderr=devnull)
    _, status, usage = os.wait4(proc.pid, 0)
    elapsed = time.perf_counter() - start
    os.close(devnull)
    code = os.waitstatus_to_exitcode(status)
    return elapsed, usage.ru_maxrss / 1024.0, code  # MiB on Linux


def main():
    go, rust, data = sys.argv[1], sys.argv[2], sys.argv[3]
    runs = int(sys.argv[4]) if len(sys.argv) > 4 else 5
    subprocess.run(["cp", "records.json", "records2.json"], cwd=data, check=True)
    with open(os.path.join(data, "records2.json"), "r+b") as f:
        f.seek(1000)
        f.write(b"9")  # one changed digit near the start
    cases = [
        ("pretty", ["-c", "never", "records.json"]),
        ("pretty+color", ["-c", "always", "records.json"]),
        ("compact", ["-c", "never", "--compact", "records.json"]),
        ("sort-keys", ["-c", "never", "--sort-keys", "records.json"]),
        ("flat", ["-c", "never", "-f", "records.json"]),
        ("typed", ["-c", "never", "-t", "records.json"]),
        ("schema", ["-c", "never", "-s", "records.json"]),
        ("path [*].address.city", ["-c", "never", "-r", "-p", "[*].address.city", "records.json"]),
        ("jsonl compact", ["-c", "never", "--compact", "records.jsonl"]),
        ("wide object", ["-c", "never", "wide.json"]),
        ("diff", ["-c", "never", "--diff", "records.json", "records2.json"]),
        ("startup (tiny input)", ["-c", "never", "-p", ".", "wide_tiny.json"]),
    ]
    with open(os.path.join(data, "wide_tiny.json"), "w") as f:
        f.write('{"a":1}')
    print(f"| case | Go time (s) | Rust time (s) | speedup | Go RSS (MiB) | Rust RSS (MiB) |")
    print(f"|---|---:|---:|---:|---:|---:|")
    for name, args in cases:
        results = {}
        for label, binary in (("go", go), ("rust", rust)):
            measure([binary] + args, data)  # warm-up (page cache)
            samples = [measure([binary] + args, data) for _ in range(runs)]
            codes = {s[2] for s in samples}
            results[label] = (
                statistics.median(s[0] for s in samples),
                max(s[1] for s in samples),
                codes,
            )
        g, r = results["go"], results["rust"]
        if g[2] != r[2]:
            print(f"warning: exit codes differ for {name}: {g[2]} vs {r[2]}", file=sys.stderr)
        print(f"| {name} | {g[0]:.3f} | {r[0]:.3f} | {g[0] / r[0]:.2f}x | {g[1]:.0f} | {r[1]:.0f} |")


if __name__ == "__main__":
    main()
