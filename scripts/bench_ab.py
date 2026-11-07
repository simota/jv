#!/usr/bin/env python3
"""A/B benchmark of two jv binaries (e.g. before and after a change).

usage: bench_ab.py A_BIN B_BIN DATA_DIR RUNS [case-filter]

DATA_DIR comes from `cargo run --release --example gen_bench_data -- DATA_DIR`
(run in rust/). The A and B runs are interleaved so that drift on a shared
machine affects both. Reports the median and max wall time, median CPU time
(user+sys) and peak RSS (MiB); output goes to /dev/null.
"""
import os, statistics, subprocess, sys, time
def measure(cmd, cwd):
    dn = os.open(os.devnull, os.O_WRONLY)
    t = time.perf_counter()
    p = subprocess.Popen(cmd, cwd=cwd, stdout=dn, stderr=dn)
    _, st, ru = os.wait4(p.pid, 0)
    el = time.perf_counter() - t
    os.close(dn)
    return el, ru.ru_utime + ru.ru_stime, ru.ru_maxrss / 1024.0, os.waitstatus_to_exitcode(st)
a, b, data = sys.argv[1], sys.argv[2], sys.argv[3]
runs = int(sys.argv[4]); flt = sys.argv[5] if len(sys.argv) > 5 else ""
if not os.path.exists(os.path.join(data, "records2.json")):
    # a copy with one changed value: the "id" of record 5
    src = open(os.path.join(data, "records.json"), "rb").read()
    i = src.index(b'"id":5,') + 5
    open(os.path.join(data, "records2.json"), "wb").write(src[:i] + b"9" + src[i + 1:])
with open(os.path.join(data, "tiny.json"), "w") as f: f.write('{"a":1}')
cases = [
 ("pretty", ["-c","never","records.json"]),
 ("pretty+color", ["-c","always","records.json"]),
 ("compact", ["-c","never","--compact","records.json"]),
 ("sort-keys", ["-c","never","--sort-keys","records.json"]),
 ("flat", ["-c","never","-f","records.json"]),
 ("typed", ["-c","never","-t","records.json"]),
 ("schema", ["-c","never","-s","records.json"]),
 ("path", ["-c","never","-r","-p","[*].address.city","records.json"]),
 ("jsonl compact", ["-c","never","--compact","records.jsonl"]),
 ("wide", ["-c","never","wide.json"]),
 ("diff", ["-c","never","--diff","records.json","records2.json"]),
 ("startup", ["-c","never","-p",".","tiny.json"]),
]
print("| case | A wall med | B wall med | Δwall | A wall max | B wall max | A cpu | B cpu | Δcpu | A RSS | B RSS | ΔRSS |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
for name, args in cases:
    if flt and flt not in name: continue
    res = {}
    for lab, bin_ in (("a", a), ("b", b)):
        measure([bin_]+args, data)
    sa, sb = [], []
    for _ in range(runs):  # interleave to cancel drift
        sa.append(measure([a]+args, data)); sb.append(measure([b]+args, data))
    for lab, s in (("a", sa), ("b", sb)):
        res[lab] = (statistics.median(x[0] for x in s), max(x[0] for x in s), statistics.median(x[1] for x in s), max(x[2] for x in s), {x[3] for x in s})
    A, B = res["a"], res["b"]
    if A[4] != B[4]: print("EXIT CODE MISMATCH", name, A[4], B[4], file=sys.stderr)
    pct = lambda x, y: f"{(y-x)/x*100:+.1f}%"
    print(f"| {name} | {A[0]:.3f} | {B[0]:.3f} | {pct(A[0],B[0])} | {A[1]:.3f} | {B[1]:.3f} | {A[2]:.3f} | {B[2]:.3f} | {pct(A[2],B[2])} | {A[3]:.0f} | {B[3]:.0f} | {pct(A[3],B[3])} |", flush=True)
