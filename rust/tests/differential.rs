//! Differential tests: the Rust binary must behave exactly like the Go
//! binary (stdout, stderr and exit status) on the same inputs.
//!
//! Set `JV_GO_BIN` to a Go build of jv to run them, e.g.
//!
//! ```sh
//! (cd .. && CGO_ENABLED=0 go build -o /tmp/jv-go ./cmd/jv)
//! JV_GO_BIN=/tmp/jv-go cargo test --test differential -- --nocapture
//! ```
//!
//! `JV_DIFF_CASES` scales the number of generated cases (default 400 per
//! generator).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod common;
use common::*;

#[derive(Debug)]
struct Run {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: Option<i32>,
}

fn exec(bin: &Path, args: &[String], stdin: &[u8], cwd: &Path) -> Run {
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {}: {e}", bin.display()));
    let mut pipe = child.stdin.take().unwrap();
    let data = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = pipe.write_all(&data);
    });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    Run {
        stdout: out.stdout,
        stderr: out.stderr,
        code: out.status.code(),
    }
}

struct Harness {
    go: PathBuf,
    rust: PathBuf,
    dir: PathBuf,
    cases: usize,
    failures: Vec<String>,
}

impl Harness {
    fn new(name: &str) -> Option<Harness> {
        let go = std::env::var_os("JV_GO_BIN")?;
        let dir = std::env::temp_dir().join(format!("jv-diff-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cases = std::env::var("JV_DIFF_CASES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(400);
        Some(Harness {
            go: PathBuf::from(go),
            rust: PathBuf::from(env!("CARGO_BIN_EXE_jv")),
            dir,
            cases,
            failures: Vec::new(),
        })
    }

    fn file(&self, name: &str, content: &[u8]) -> String {
        std::fs::write(self.dir.join(name), content).unwrap();
        name.to_string()
    }

    fn check(&mut self, args: &[String], stdin: &[u8]) {
        let g = exec(&self.go, args, stdin, &self.dir);
        let r = exec(&self.rust, args, stdin, &self.dir);
        if g.stdout != r.stdout || g.stderr != r.stderr || g.code != r.code {
            let show = |b: &[u8]| {
                let s = String::from_utf8_lossy(b);
                if s.len() > 600 {
                    format!("{}…", s.chars().take(600).collect::<String>())
                } else {
                    s.into_owned()
                }
            };
            self.failures.push(format!(
                "args {args:?}\nstdin {:?}\n  go:   code {:?} stdout {:?} stderr {:?}\n  rust: code {:?} stdout {:?} stderr {:?}",
                show(stdin),
                g.code,
                show(&g.stdout),
                show(&g.stderr),
                r.code,
                show(&r.stdout),
                show(&r.stderr)
            ));
        }
    }

    /// Like [`Self::check`] with stderr merged into stdout (`2>&1`), which
    /// also compares the order in which the two streams are written.
    fn check_merged(&mut self, args: &[String], stdin: &[u8]) {
        let merged = |bin: &Path| {
            let mut full = vec![
                "-c".to_string(),
                r#""$0" "$@" 2>&1"#.to_string(),
                bin.display().to_string(),
            ];
            full.extend_from_slice(args);
            exec(Path::new("/bin/sh"), &full, stdin, &self.dir)
        };
        let (g, r) = (merged(&self.go), merged(&self.rust));
        if g.stdout != r.stdout || g.code != r.code {
            self.failures.push(format!(
                "merged output differs for {args:?}\n  go:   {:?}\n  rust: {:?}",
                String::from_utf8_lossy(&g.stdout),
                String::from_utf8_lossy(&r.stdout)
            ));
        }
    }

    fn finish(self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        if !self.failures.is_empty() {
            let n = self.failures.len();
            let shown: Vec<_> = self.failures.into_iter().take(15).collect();
            panic!("{n} differential mismatches:\n\n{}", shown.join("\n\n"));
        }
    }
}

macro_rules! harness {
    ($name:expr) => {
        match Harness::new($name) {
            Some(h) => h,
            None => {
                eprintln!("JV_GO_BIN not set; skipping differential test");
                return;
            }
        }
    };
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Path expressions: real paths of the document plus variations and junk.
fn gen_path(rng: &mut Rng, doc_paths: &[String]) -> String {
    let base = if doc_paths.is_empty() || rng.chance(20) {
        String::new()
    } else {
        doc_paths[rng.below(doc_paths.len())].clone()
    };
    let tweaks = [
        "",
        "",
        "",
        "[*]",
        ".*",
        "[-1]",
        "[0]",
        "[ 1 ]",
        "[+0]",
        ".missing",
        "[\"a\"]",
        ".\"a.b\"",
        "[",
        "]",
        "..",
        "[x]",
        "[\"unterminated",
        "$",
        " ",
        ".[0]",
        "[\"\\u00e9\"]",
        "[\"\\x\"]",
        ".kebab-key",
        "[99999999999999999999]",
        ".é",
        "[*].id",
        "[-9]",
        ".\"a\tb\"",
    ];
    let mut p = base;
    let n = rng.below(3);
    for _ in 0..n {
        p.push_str(rng.pick(&tweaks));
    }
    if rng.chance(15) {
        p = p.trim_start_matches('$').to_string();
    }
    if rng.chance(10) {
        p = format!("  {p} ");
    }
    p
}

/// Paths of every node, collected with the Rust library (they are checked
/// against Go through the CLI output anyway).
fn node_paths(doc: &str) -> Vec<String> {
    let Ok(mut d) = jv::json::parse_bytes(doc.as_bytes()) else {
        return Vec::new();
    };
    let root = d.single_root();
    let mut out = Vec::new();
    d.walk(root, |n| {
        out.push(d.path(n));
        true
    });
    out
}

const MODES: &[&[&str]] = &[
    &[],
    &["--compact"],
    &["-f"],
    &["-t"],
    &["-s"],
    &["--sort-keys"],
    &["--sort-keys", "-f"],
    &["-r"],
    &["--compact", "-r"],
];

#[test]
fn formatting_matches_go() {
    let mut h = harness!("fmt");
    let mut rng = Rng(0x5eed_0001);
    for _ in 0..h.cases {
        let doc = gen_document(&mut rng);
        let mut a = args(&["-c", if rng.chance(20) { "always" } else { "never" }]);
        a.extend(rng.pick(MODES).iter().map(|s| s.to_string()));
        h.check(&a, doc.as_bytes());
    }
    h.finish();
}

#[test]
fn path_queries_match_go() {
    let mut h = harness!("path");
    let mut rng = Rng(0x5eed_0002);
    for _ in 0..h.cases {
        let doc = gen_document(&mut rng);
        let paths = node_paths(&doc);
        let mut a = args(&["-c", "never"]);
        a.extend(rng.pick(MODES).iter().map(|s| s.to_string()));
        a.push("-p".into());
        a.push(gen_path(&mut rng, &paths));
        h.check(&a, doc.as_bytes());
    }
    h.finish();
}

#[test]
fn syntax_errors_match_go() {
    let mut h = harness!("errors");
    let mut rng = Rng(0x5eed_0003);
    for _ in 0..h.cases {
        let doc = gen_document(&mut rng);
        let bad = mutate(&mut rng, &doc);
        h.check(&args(&["-c", "never", "--compact"]), &bad);
    }
    h.finish();
}

#[test]
fn diffs_match_go() {
    let mut h = harness!("diff");
    let mut rng = Rng(0x5eed_0004);
    for i in 0..h.cases {
        let a = gen_document(&mut rng);
        let b = if rng.chance(30) {
            gen_document(&mut rng)
        } else {
            // a small edit of `a` that is still valid JSON (retry a few times)
            let mut candidate = a.clone();
            for _ in 0..10 {
                let m = mutate(&mut rng, &a);
                if jv::json::parse_bytes(&m).is_ok() {
                    candidate = String::from_utf8_lossy(&m).into_owned();
                    break;
                }
            }
            candidate
        };
        let fa = h.file(&format!("a{i}.json"), a.as_bytes());
        let fb = h.file(&format!("b{i}.json"), b.as_bytes());
        let mut list = args(&[
            "-c",
            if rng.chance(20) { "always" } else { "never" },
            "--diff",
        ]);
        if rng.chance(25) {
            let paths = node_paths(&a);
            list.push("-p".into());
            list.push(gen_path(&mut rng, &paths));
        }
        if rng.chance(30) {
            list.push(fa);
            list.push("-".into());
            h.check(&list, b.as_bytes());
        } else {
            list.push(fa);
            list.push(fb);
            h.check(&list, b"");
            if cfg!(unix) && i % 4 == 0 {
                h.check_merged(&list, b"");
            }
        }
    }
    h.finish();
}

#[test]
fn command_line_handling_matches_go() {
    let mut h = harness!("cli");
    let sample = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/sample.json"
    ))
    .unwrap();
    let s = h.file("sample.json", &sample);
    let a = h.file("a.json", br#"{"n":1,"list":[1,2],"cfg":{"on":true}}"#);
    let b = h.file(
        "b.json",
        br#"{"cfg":{"on":false},"n":1.0,"list":[1],"new":"v"}"#,
    );
    h.file("bad.json", b"{\n  \"a\": 1,\n  \"b\": \n}");
    h.file("empty.json", b"  \n");
    std::fs::create_dir_all(h.dir.join("adir")).unwrap();
    let cases: Vec<Vec<&str>> = vec![
        vec!["-h"],
        vec!["--help"],
        vec!["-h", "a", "b", "c"],
        vec!["--help=false", "-c", "never", &s],
        vec!["-h", "--bogus"],
        vec!["--bogus"],
        vec!["--bogus=1"],
        vec!["-x"],
        vec!["-rx"],
        vec!["-xr"],
        vec!["-r="],
        vec!["-é"],
        vec!["-p"],
        vec!["-rp"],
        vec!["--path"],
        vec!["---x"],
        vec!["--=x"],
        vec!["-d", "abc", &s],
        vec!["-d", "99999999999999999999", &s],
        vec!["-d", "0x10", "-c", "never", &s],
        vec!["--compact=yes", &s],
        vec!["--compact=1", "-c", "never", &s],
        vec!["--compact=F", "-c", "never", &s],
        vec!["-r=false", "-p", ".name", &s],
        vec!["-p.name", &s, "-c=never"],
        vec!["-cnever", "-rp.address.city", &s],
        vec!["-c", "never", "--", &s],
        vec!["-c", "never", "--", "-x"],
        vec!["a", "b", "c"],
        vec![&s, &s],
        vec!["--diff", &s],
        vec!["--diff", "-", "-"],
        vec!["--diff", "-c", "never", &a, &b],
        vec!["--diff", "-c", "always", &a, &b],
        vec!["--diff", &a, &a],
        vec!["--diff", "-p", ".cfg", &a, &b],
        vec!["--diff", "-p", ".x", &a, &b],
        vec!["--diff", "-p", ".list[*]", &a, &b],
        vec!["--diff", "-p", ".a[", &a, &b],
        vec!["--diff", "-p", ".a[", "missing.json", &b],
        vec!["--diff", "--theme", "x", &a, &b],
        vec!["-i", "-n", &s],
        vec!["--theme", "blue", &s],
        vec!["--theme", "light", "-c", "never", &s],
        vec!["-c", "rainbow", &s],
        vec!["-c", "auto", &s],
        vec!["missing.json"],
        vec!["adir"],
        vec!["bad.json"],
        vec!["empty.json"],
        vec!["-c", "never", "-p", "", &s],
        vec!["-c", "never", "-p", ".projects[*].name", &s],
        vec!["-c", "never", "-s", "-p", ".projects", &s],
        vec!["-c", "never", "-t", "-p", ".address", &s],
        vec!["-c", "never", "-f", "-p", ".projects[-1]", &s],
        vec!["-p", ".missing", &s],
        vec!["-p", "$.a[x", &s],
        vec!["-p", "$$", &s],
        vec!["-p", "]", &s],
        vec!["-p", "é]", &s],
        vec!["-p", ".\"a\tb\"", &s],
        vec!["-p", "  ", "-c", "never", &s],
        vec!["-i", "-p", ".zzz", &s],
        vec!["-test.v", "-c", "never", &s],
        vec!["-c", "never", "--sort-keys", "-t", &s],
        vec!["", "-c", "never"],
        vec!["-", "-c", "never"],
    ];
    for c in cases {
        h.check(&args(&c), b"[1, {\"stdin\": true}]");
        if cfg!(unix) {
            h.check_merged(&args(&c), b"[1, {\"stdin\": true}]");
        }
    }
    h.finish();
}

#[test]
fn large_and_deep_documents_match_go() {
    let mut h = harness!("large");
    let mut rng = Rng(0x5eed_0005);
    let big: Vec<String> = (0..3000).map(|_| gen_value(&mut rng, 1)).collect();
    let big = format!("[{}]", big.join(","));
    for mode in MODES {
        let mut a = args(&["-c", "never"]);
        a.extend(mode.iter().map(|s| s.to_string()));
        h.check(&a, big.as_bytes());
    }
    let wide = format!(
        "{{{}}}",
        (0..500)
            .map(|i| format!("\"k{}\":{i}", i % 300))
            .collect::<Vec<_>>()
            .join(",")
    );
    h.check(&args(&["-c", "never", "--compact"]), wide.as_bytes());
    for depth in [100, 9999, 10000, 10001] {
        let deep = "[".repeat(depth) + "1" + &"]".repeat(depth);
        h.check(&args(&["-c", "never", "--compact"]), deep.as_bytes());
        let deep_obj = "{\"a\":".repeat(depth) + "1" + &"}".repeat(depth);
        h.check(&args(&["-c", "never", "-s"]), deep_obj.as_bytes());
    }
    h.finish();
}
