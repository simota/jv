//! End-to-end tests of the command line (port of the Go `root_test.go`,
//! plus process-level checks of the real binary).

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use jv::cli::{Env, execute};
use jv::json::{Document, NodeId, NodeType};
use jv::tui;

const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/sample.json");

struct Output {
    stdout: String,
    stderr: String,
    code: i32,
}

/// What the stubbed TUI received.
#[derive(Default)]
struct TuiCall {
    root_type: Option<NodeType>,
    root_children: usize,
    focus_paths: Vec<String>,
    opts_depth: usize,
    color: bool,
}

fn run_with(input: &str, no_color: bool, args: &[&str], tui_call: &mut TuiCall) -> Output {
    let mut stdin = input.as_bytes();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut run_tui = |doc: Document, root: NodeId, opts: tui::Options| {
        tui_call.root_type = Some(doc.node_type(root));
        tui_call.root_children = doc.children(root).len();
        tui_call.focus_paths = opts.focus.iter().map(|&n| doc.path(n)).collect();
        tui_call.opts_depth = opts.depth;
        tui_call.color = opts.color;
        Ok(())
    };
    let mut env = Env {
        stdin: &mut stdin,
        // like the Go tests: stdin counts as a terminal when no input is given
        stdin_is_terminal: input.is_empty(),
        stdout: &mut stdout,
        stdout_is_terminal: false,
        stderr: &mut stderr,
        no_color,
        completion_descriptions: None,
        run_tui: &mut run_tui,
    };
    let code = execute(args.iter().map(OsString::from).collect(), &mut env);
    Output {
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
        code,
    }
}

fn run(input: &str, args: &[&str]) -> Output {
    run_with(input, false, args, &mut TuiCall::default())
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("jv-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    fn file(&self, name: &str, content: &str) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, content).unwrap();
        p.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn pretty_default_keeps_input_order() {
    let out = run("", &["-c", "never", SAMPLE]);
    assert_eq!(out.code, 0);
    assert!(
        out.stdout
            .starts_with("{\n  \"name\": \"John Doe\",\n  \"age\": 30,"),
        "{}",
        out.stdout
    );
    let sorted = run("", &["-c", "never", "--sort-keys", SAMPLE]);
    assert!(
        sorted.stdout.starts_with("{\n  \"active\": true,"),
        "{}",
        sorted.stdout
    );
}

#[test]
fn path_extraction() {
    let cases: &[(&[&str], &str)] = &[
        (&["-p", ".projects[*].name"], "\"Alpha\"\n\"Beta\"\n"),
        (&["-r", "-p", "$.address.city"], "Tokyo\n"),
        (
            &["--compact", "-p", ".address.lines"],
            "[\"1-1-1 Chiyoda\",\"Building 2\"]\n",
        ),
        (
            &["-f", "-p", ".projects[-1]"],
            "$.id = 2\n$.name = \"Beta\"\n$.status = null\n",
        ),
        (
            &["-s", "-p", ".projects"],
            "[\n  {\n    \"id\": number,\n    \"name\": string,\n    \"status\": string | null\n  }\n]\n",
        ),
    ];
    for (args, want) in cases {
        let mut full = vec!["-c", "never"];
        full.extend_from_slice(args);
        full.push(SAMPLE);
        let out = run("", &full);
        assert_eq!(
            (out.code, out.stdout.as_str()),
            (0, *want),
            "{args:?}: {}",
            out.stderr
        );
    }
}

#[test]
fn path_errors() {
    let out = run("", &["-p", ".missing", SAMPLE]);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("path not found: .missing"),
        "{}",
        out.stderr
    );
    let out = run("", &["-p", ".a[", SAMPLE]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("invalid path"), "{}", out.stderr);
    assert!(!out.stderr.contains("Usage:") && !out.stdout.contains("Usage:"));
}

#[test]
fn stdin_json_lines() {
    let lines = "{\"a\":1}\n{\"a\":2,\"b\":\"x\"}\n";
    let out = run(lines, &["-c", "never", "--compact"]);
    assert_eq!(
        (out.code, out.stdout.as_str()),
        (0, "{\"a\":1}\n{\"a\":2,\"b\":\"x\"}\n")
    );
    assert_eq!(
        run(lines, &["-c", "never", "-r", "-p", ".a"]).stdout,
        "1\n2\n"
    );
    assert_eq!(
        run(lines, &["-c", "never", "-s"]).stdout,
        "{\n  \"a\": number,\n  \"b\"?: string\n}\n"
    );
    assert_eq!(
        run("[1]", &["-c", "never", "--compact", "-"]).stdout,
        "[1]\n"
    );
}

#[test]
fn syntax_error_reports_position() {
    let dir = TempDir::new("syntax");
    let p = dir.file("bad.json", "{\n  \"a\": 1,\n  \"b\": \n}");
    let out = run("", &[&p]);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr
            .contains("bad.json: invalid JSON at line 4, column 1"),
        "{}",
        out.stderr
    );
}

#[test]
fn no_color_env() {
    let out = run_with(
        "",
        true,
        &["--compact", "-p", ".age", SAMPLE],
        &mut TuiCall::default(),
    );
    assert_eq!(out.stdout, "30\n");
    let out = run_with(
        "",
        true,
        &["--compact", "-c", "always", "-p", ".age", SAMPLE],
        &mut TuiCall::default(),
    );
    assert!(
        out.stdout.contains("\x1b["),
        "explicit --color always should win: {:?}",
        out.stdout
    );
    // default is "always", even when stdout is not a terminal
    assert!(
        run("", &["--compact", "-p", ".age", SAMPLE])
            .stdout
            .contains("\x1b[33m30")
    );
    assert_eq!(
        run("", &["--compact", "-c", "auto", "-p", ".age", SAMPLE]).stdout,
        "30\n"
    );
}

#[test]
fn diff() {
    let dir = TempDir::new("diff");
    let a = dir.file(
        "a.json",
        r#"{"name":"x","n":1,"list":[1,2],"cfg":{"on":true}}"#,
    );
    let b = dir.file(
        "b.json",
        r#"{"cfg":{"on":false},"name":"x","n":1.0,"list":[1],"new":"v"}"#,
    );
    let out = run("", &["-c", "never", "--diff", &a, &b]);
    assert_eq!(out.code, 1);
    assert_eq!(
        out.stdout,
        "- $.list[1]: 2\n~ $.cfg.on: true -> false\n+ $.new: \"v\"\n"
    );
    assert!(
        out.stderr.contains("1 changed, 1 added, 1 removed"),
        "{}",
        out.stderr
    );

    let out = run("", &["--diff", &a, &a]);
    assert_eq!((out.code, out.stdout.as_str()), (0, ""));

    let out = run(
        r#"{"cfg":{"on":true}}"#,
        &["-c", "never", "--diff", "-p", ".cfg", &a, "-"],
    );
    assert_eq!((out.code, out.stdout.as_str()), (0, ""), "{}", out.stderr);

    let out = run("", &["--diff", &a]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("requires two inputs"));

    let out = run("", &[&a, &b]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("--diff"));

    let out = run("", &["--diff", "-", "-"]);
    assert_eq!(
        out.stderr,
        "Error: only one --diff input can be read from stdin\n"
    );
    let out = run("", &["--diff", "-p", ".x", &a, &b]);
    assert_eq!(out.stderr, format!("Error: path not found in {a}: .x\n"));
    let out = run("", &["--diff", "-p", ".list[*]", &a, &b]);
    assert_eq!(
        out.stderr,
        "Error: path .list[*] matches 2 values; --diff needs a single value\n"
    );
}

#[test]
fn interactive_passes_focus() {
    let mut call = TuiCall::default();
    let out = run_with(
        "{\"a\":1}\n{\"a\":2}\n",
        false,
        &["-i", "-p", ".a"],
        &mut call,
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(
        call.root_type,
        Some(NodeType::Array),
        "multi-doc root not wrapped"
    );
    assert_eq!(call.root_children, 2);
    assert_eq!(call.focus_paths, ["$[0].a", "$[1].a"]);
    assert_eq!(call.opts_depth, 2);
    assert!(call.color);

    let out = run("{}", &["-i", "-p", ".zzz"]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("path not found"), "{}", out.stderr);

    let mut call = TuiCall::default();
    run_with("{}", true, &["-i", "-d", "-3"], &mut call);
    assert_eq!(call.opts_depth, 0, "negative depth is clamped");
    assert!(!call.color, "NO_COLOR applies to the viewer too");
}

#[test]
fn flag_validation() {
    for args in [&["-i", "-n"][..], &["--theme", "blue"], &["-c", "rainbow"]] {
        let out = run("{}", args);
        assert_eq!(out.code, 1, "{args:?}");
        assert!(!out.stderr.is_empty());
    }
    assert_eq!(
        run("{}", &["--theme", "blue"]).stderr,
        "Error: invalid theme: blue\n"
    );
    assert_eq!(
        run("{}", &["-c", "rainbow"]).stderr,
        "Error: invalid color mode: rainbow\n"
    );
    assert_eq!(
        run("{}", &["-i", "-n"]).stderr,
        "Error: cannot use --interactive and --no-interactive together\n"
    );
}

#[test]
fn usage_errors_print_usage_to_stdout() {
    let out = run("{}", &["--bogus"]);
    assert_eq!(out.code, 1);
    assert_eq!(out.stderr, "Error: unknown flag: --bogus\n");
    assert!(
        out.stdout
            .starts_with("Usage:\n  jv [OPTIONS] [FILE]\n  jv --diff OLD NEW [flags]\n")
    );
    assert!(
        out.stdout
            .ends_with("  -t, --type             Show type hints\n\n")
    );

    let out = run("{}", &["a", "b", "c"]);
    assert_eq!(out.stderr, "Error: accepts at most 2 arg(s), received 3\n");
    assert!(out.stdout.starts_with("Usage:"));
}

#[test]
fn help() {
    let out = run("", &["-h"]);
    assert_eq!(out.code, 0);
    assert!(
        out.stdout
            .starts_with("JSON viewer for the terminal.\n\nReads a JSON document")
    );
    assert!(out.stdout.contains("\n\nUsage:\n"));
    assert!(
        out.stdout
            .contains("  -h, --help             help for jv\n")
    );
    assert_eq!(out.stderr, "");
    // help wins over argument-count errors, but not over flag errors
    assert_eq!(run("", &["-h", "a", "b", "c"]).code, 0);
    assert_eq!(run("", &["-h", "--bogus"]).code, 1);
}

#[test]
fn input_errors() {
    let out = run("", &[]);
    assert_eq!(
        out.stderr,
        "Error: no input provided. Try: jv path/to.json | cat file.json | jv | echo '{}' | jv\n"
    );
    // OS error texts are platform specific (Go's are too)
    #[cfg(unix)]
    {
        assert_eq!(
            run("", &["/nonexistent/x.json"]).stderr,
            "Error: open /nonexistent/x.json: no such file or directory\n"
        );
        let dir = TempDir::new("isdir");
        let d = dir.0.to_string_lossy().into_owned();
        assert_eq!(
            run("", &[&d]).stderr,
            format!("Error: read {d}: is a directory\n")
        );
    }
    let missing = run("", &["missing-file-for-jv-test.json"]);
    assert_eq!(missing.code, 1);
    assert!(
        missing
            .stderr
            .starts_with("Error: open missing-file-for-jv-test.json: ")
    );
    assert_eq!(
        run("  \n ", &[]).stderr,
        "Error: no JSON value found in input\n"
    );
    assert_eq!(
        run("[1,", &[]).stderr,
        "Error: stdin: invalid JSON at line 1, column 4: unexpected end of input\n"
    );
}

#[test]
fn output_modes() {
    let doc = r#"{"a":[1,{"b":null}],"c":"x"}"#;
    assert_eq!(
        run(doc, &["-c", "never", "-t"]).stdout,
        "root {2}  object\n|- \"a\" [2]  array\n|  |- [0]: 1  number\n|  `- [1] {1}  object\n|     `- \"b\": null  null\n`- \"c\": \"x\"  string\n"
    );
    // precedence: flat > type > compact > pretty, raw only for pretty/compact
    assert_eq!(
        run(doc, &["-c", "never", "-f", "-t", "--compact"])
            .stdout
            .lines()
            .count(),
        3
    );
    assert_eq!(
        run(doc, &["-c", "never", "-t", "-r", "-p", ".c"]).stdout,
        "`- \"c\": \"x\"  string\n"
    );
    assert_eq!(
        run(doc, &["-c", "never", "--compact", "-r", "-p", ".c"]).stdout,
        "x\n"
    );
}

// --- process-level checks of the real binary ---

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jv"))
}

fn run_bin(args: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = bin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let data = input.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    out
}

#[test]
fn binary_reads_piped_stdin_and_sets_exit_codes() {
    let out = run_bin(&["-c", "never", "--compact"], b"{\"a\" : [1, 2]}");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"{\"a\":[1,2]}\n");
    let out = run_bin(&["-p", ".nope"], b"{}");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stderr, b"Error: path not found: .nope\n");
}

#[test]
fn binary_handles_maximum_nesting() {
    let depth = jv::json::MAX_NESTING;
    let doc = "[".repeat(depth) + &"]".repeat(depth);
    for mode in [
        &["-c", "never"][..],
        &["-c", "never", "--compact"],
        &["-s"],
        &["-t"],
        &["-f"],
    ] {
        let out = run_bin(mode, doc.as_bytes());
        assert!(
            out.status.success(),
            "{mode:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = run_bin(&["--diff", "-", jv_sample()], doc.as_bytes());
    assert_eq!(out.status.code(), Some(1));
    let too_deep = "[".repeat(depth + 1) + &"]".repeat(depth + 1);
    let out = run_bin(&[], too_deep.as_bytes());
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("exceeded max nesting depth"));
}

#[cfg(unix)]
#[test]
fn binary_diff_summary_follows_changes_on_shared_terminal() {
    // with `2>&1` the summary (stderr) must come after the changes (stdout)
    let dir = TempDir::new("order");
    let a = dir.file("a.json", r#"{"x":1}"#);
    let b = dir.file("b.json", r#"{"x":2}"#);
    let out = Command::new("/bin/sh")
        .args([
            "-c",
            r#""$0" "$@" 2>&1"#,
            env!("CARGO_BIN_EXE_jv"),
            "-c",
            "never",
            "--diff",
            &a,
            &b,
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "~ $.x: 1 -> 2\njv: 1 changed\n"
    );
}

fn jv_sample() -> &'static str {
    SAMPLE
}

#[cfg(unix)]
#[test]
fn binary_exits_quietly_when_stdout_closes() {
    // `jv big.json | head -c 10`: the reader goes away early
    let big = format!("[{}]", vec!["\"0123456789abcdef\""; 200_000].join(","));
    let mut child = bin()
        .args(["-c", "never"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(big.as_bytes()).unwrap();
    drop(stdin);
    drop(child.stdout.take()); // close the read end
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(jv::cli::EXIT_BROKEN_PIPE));
    assert_eq!(out.stderr, b"");
}

#[test]
fn shell_completion() {
    let out = run("", &["completion", "bash"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.starts_with("# bash completion V2 for jv"));
    assert!(
        out.stdout
            .contains("requestComp=\"${words[0]} __complete ${args[*]}\"")
    );
    let nodesc = run("", &["completion", "bash", "--no-descriptions"]).stdout;
    assert!(nodesc.contains("requestComp=\"${words[0]} __completeNoDesc ${args[*]}\""));
    for shell in ["zsh", "fish", "powershell"] {
        let out = run("", &["completion", shell]);
        assert_eq!(out.code, 0, "{shell}");
        assert!(out.stdout.contains("__complete"), "{shell}");
    }
    let help = run("", &["completion"]);
    assert!(
        help.stdout
            .starts_with("Generate the autocompletion script for jv for the specified shell.")
    );
    let bad = run("", &["completion", "bash", "extra"]);
    assert_eq!(bad.code, 1);
    assert_eq!(
        bad.stderr,
        "Error: unknown command \"extra\" for \"jv completion bash\"\n"
    );

    let flags = run("", &["__complete", "-r", "--c"]);
    assert_eq!(
        flags.stdout,
        "--color\tColor (auto/always/never); NO_COLOR is honored when not set\n--compact\tPrint each document on a single line\n:4\n"
    );
    assert_eq!(
        flags.stderr,
        "Completion ended with directive: ShellCompDirectiveNoFileComp\n"
    );
    assert_eq!(
        run("", &["__completeNoDesc", "--sort"]).stdout,
        "--sort-keys\n:4\n"
    );
    // values and file names are left to the shell
    assert_eq!(run("", &["__complete", "-p", ""]).stdout, ":0\n");
    let unknown = run("", &["__complete", "--bogus=x"]);
    assert!(
        unknown
            .stderr
            .starts_with("[Debug] [Error] Subcommand 'jv' does not support flag 'bogus'\n")
    );
    assert_eq!(run("", &["__complete"]).code, 1);
}
