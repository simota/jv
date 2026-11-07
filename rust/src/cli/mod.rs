//! The `jv` command line.
//!
//! [`execute`] runs one invocation against injectable I/O so that the whole
//! command can be tested in-process; [`main`] wires it to the real process.

mod args;
mod help;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

pub use args::{Options, Parsed, parse};

use crate::diff;
use crate::format::{
    FlatFormatter, Formatter, PrettyFormatter, SchemaFormatter, TypedFormatter, format_diff,
};
use crate::json::{Document, NodeId, ParseError, parse_bytes};
use crate::path::Query;
use crate::tui;

/// Exit status used when stdout is closed early (e.g. `jv big.json | head`).
/// The Go binary is killed by SIGPIPE in that case, which shells report as
/// 128 + 13.
pub const EXIT_BROKEN_PIPE: i32 = 141;

/// Process environment and streams for one invocation.
pub struct Env<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdin_is_terminal: bool,
    pub stdout: &'a mut dyn Write,
    pub stdout_is_terminal: bool,
    pub stderr: &'a mut dyn Write,
    /// `NO_COLOR` is set to a non-empty value.
    pub no_color: bool,
    /// Starts the interactive viewer.
    pub run_tui: &'a mut dyn FnMut(Document, NodeId, tui::Options) -> io::Result<()>,
}

/// Runs the real process: arguments, stdio, terminal detection, and the TUI.
pub fn main() -> i32 {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let stdout = io::stdout();
    let stdout_is_terminal = stdout.is_terminal();
    let mut out = io::BufWriter::with_capacity(64 * 1024, stdout.lock());
    let mut err = io::stderr();
    let mut run_tui = |doc, root, opts| tui::run(doc, root, opts);
    let mut stdin = stdin.lock();
    let mut env = Env {
        stdin: &mut stdin,
        stdin_is_terminal,
        stdout: &mut out,
        stdout_is_terminal,
        stderr: &mut err,
        no_color: std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
        run_tui: &mut run_tui,
    };
    execute(args, &mut env)
}

/// Why a run failed.
#[derive(Debug)]
enum Failure {
    /// A flag/argument error: cobra prints the usage too.
    Usage(String),
    /// A runtime error, printed as `Error: ...`.
    Error(String),
    /// Exit with the given status without a message.
    Exit(i32),
}

impl From<String> for Failure {
    fn from(s: String) -> Self {
        Failure::Error(s)
    }
}

impl From<&str> for Failure {
    fn from(s: &str) -> Self {
        Failure::Error(s.to_string())
    }
}

/// Runs `jv` with `args` (without the program name) and returns the exit
/// status.
pub fn execute(args: Vec<OsString>, env: &mut Env<'_>) -> i32 {
    let result = parse(args)
        .map_err(Failure::Usage)
        .and_then(|parsed| dispatch(parsed, env));
    let flushed = env.stdout.flush();
    let result = match (result, flushed) {
        (Ok(()), Err(e)) => Err(write_failure(e)),
        (r, _) => r,
    };
    match result {
        Ok(()) => 0,
        Err(Failure::Exit(code)) => code,
        Err(Failure::Usage(msg)) => {
            let _ = writeln!(env.stdout, "{}", help::USAGE);
            let _ = env.stdout.flush();
            let _ = writeln!(env.stderr, "Error: {msg}");
            1
        }
        Err(Failure::Error(msg)) => {
            let _ = writeln!(env.stderr, "Error: {msg}");
            1
        }
    }
}

fn dispatch(parsed: Parsed, env: &mut Env<'_>) -> Result<(), Failure> {
    let Parsed { mut options, args } = parsed;
    if options.help {
        write!(env.stdout, "{}\n\n{}", help::LONG, help::USAGE).map_err(write_failure)?;
        return Ok(());
    }
    if args.len() > 2 {
        return Err(Failure::Usage(format!(
            "accepts at most 2 arg(s), received {}",
            args.len()
        )));
    }
    if !options.color_changed && env.no_color {
        options.color = "never".into();
    }
    if options.diff {
        if args.len() != 2 {
            return Err("--diff requires two inputs: jv --diff OLD NEW".into());
        }
        return run_diff(&options, &args[0], &args[1], env);
    }
    if args.len() == 2 {
        return Err("two inputs given; did you mean: jv --diff OLD NEW".into());
    }
    run(&options, args.first(), env)
}

/// Checks flag combinations and values (a negative depth is clamped later).
fn validate(opts: &Options) -> Result<(), Failure> {
    if opts.interactive && opts.no_interactive {
        return Err("cannot use --interactive and --no-interactive together".into());
    }
    if opts.theme != "dark" && opts.theme != "light" {
        return Err(format!("invalid theme: {}", opts.theme).into());
    }
    if !matches!(opts.color.as_str(), "auto" | "always" | "never") {
        return Err(format!("invalid color mode: {}", opts.color).into());
    }
    Ok(())
}

fn run(opts: &Options, file: Option<&OsString>, env: &mut Env<'_>) -> Result<(), Failure> {
    validate(opts)?;
    let query = if opts.path.is_empty() {
        None
    } else {
        Some(Query::compile(&opts.path).map_err(|e| e.to_string())?)
    };
    let mut doc = load_documents(file, opts.sort_keys, env)?;
    let docs = doc.roots().to_vec();
    let interactive = opts.interactive;
    let color = color_enabled(&opts.color, interactive, env.stdout_is_terminal);

    if interactive {
        let root = doc.single_root();
        let mut focus = Vec::new();
        if let Some(q) = &query {
            for &d in &docs {
                focus.extend(q.find(&doc, d));
            }
            if focus.is_empty() {
                return Err(format!("path not found: {q}").into());
            }
        }
        let tui_opts = tui::Options {
            depth: opts.depth.max(0).try_into().unwrap_or(usize::MAX),
            theme: if opts.theme == "light" {
                tui::Theme::Light
            } else {
                tui::Theme::Dark
            },
            color,
            show_types: opts.show_type,
            focus,
            focus_label: opts.path.clone(),
        };
        return (env.run_tui)(doc, root, tui_opts).map_err(|e| Failure::Error(e.to_string()));
    }

    let targets = match &query {
        None => docs,
        Some(q) => {
            let found: Vec<NodeId> = docs.iter().flat_map(|&d| q.find(&doc, d)).collect();
            if found.is_empty() {
                return Err(format!("path not found: {q}").into());
            }
            found
        }
    };
    if opts.schema {
        let out = SchemaFormatter::new(color).format_many(&doc, &targets);
        return env.stdout.write_all(out.as_bytes()).map_err(write_failure);
    }
    let formatter = select_formatter(opts, color);
    let mut buf = String::new();
    for id in targets {
        buf.clear();
        formatter.format_into(&mut buf, &doc, id);
        env.stdout
            .write_all(buf.as_bytes())
            .map_err(write_failure)?;
    }
    Ok(())
}

fn select_formatter(opts: &Options, color: bool) -> Box<dyn Formatter> {
    if opts.flat {
        Box::new(FlatFormatter::new(color))
    } else if opts.show_type {
        Box::new(TypedFormatter::new(color))
    } else if opts.compact {
        Box::new(PrettyFormatter::compact(color).with_raw(opts.raw))
    } else {
        Box::new(PrettyFormatter::new(color).with_raw(opts.raw))
    }
}

fn run_diff(
    opts: &Options,
    old: &OsString,
    new: &OsString,
    env: &mut Env<'_>,
) -> Result<(), Failure> {
    validate(opts)?;
    if old == "-" && new == "-" {
        return Err("only one --diff input can be read from stdin".into());
    }
    let (da, a) = load_diff_input(opts, old, env)?;
    let (db, b) = load_diff_input(opts, new, env)?;
    let changes = diff::compare(&da, a, &db, b);
    let color = color_enabled(&opts.color, false, env.stdout_is_terminal);
    let out = format_diff(&changes, &da, &db, color);
    env.stdout
        .write_all(out.as_bytes())
        .map_err(write_failure)?;
    if !changes.is_empty() {
        let _ = writeln!(env.stderr, "jv: {}", diff::summary(&changes));
        return Err(Failure::Exit(1));
    }
    Ok(())
}

fn load_diff_input(
    opts: &Options,
    file: &OsString,
    env: &mut Env<'_>,
) -> Result<(Document, NodeId), Failure> {
    let mut doc = load_documents(Some(file), false, env)?;
    let root = doc.single_root();
    if opts.path.is_empty() {
        return Ok((doc, root));
    }
    let q = Query::compile(&opts.path).map_err(|e| e.to_string())?;
    let found = q.find(&doc, root);
    match found.len() {
        0 => Err(format!(
            "path not found in {}: {}",
            display_name(Some(file)),
            opts.path
        )
        .into()),
        1 => Ok((doc, found[0])),
        n => Err(format!(
            "path {} matches {n} values; --diff needs a single value",
            opts.path
        )
        .into()),
    }
}

fn display_name(file: Option<&OsString>) -> String {
    match file {
        None => "stdin".into(),
        Some(f) if f.is_empty() || f == "-" => "stdin".into(),
        Some(f) => f.to_string_lossy().into_owned(),
    }
}

fn load_documents(
    file: Option<&OsString>,
    sort_keys: bool,
    env: &mut Env<'_>,
) -> Result<Document, Failure> {
    let data = read_input(file, env)?;
    let mut doc = parse_bytes(&data).map_err(|e| match e {
        ParseError::Syntax(se) => format!("{}: {se}", display_name(file)),
        ParseError::Empty => e.to_string(),
    })?;
    if sort_keys {
        for root in doc.roots().to_vec() {
            doc.sort_keys(root);
        }
    }
    Ok(doc)
}

fn read_input(file: Option<&OsString>, env: &mut Env<'_>) -> Result<Vec<u8>, Failure> {
    match file {
        Some(f) if !f.is_empty() && f != "-" => read_file(Path::new(f)).map_err(Failure::Error),
        _ if file.is_some_and(|f| f == "-") || !env.stdin_is_terminal => {
            let mut data = Vec::new();
            env.stdin
                .read_to_end(&mut data)
                .map_err(|e| format!("read /dev/stdin: {}", GoIoError(&e)))?;
            Ok(data)
        }
        _ => Err(
            "no input provided. Try: jv path/to.json | cat file.json | jv | echo '{}' | jv".into(),
        ),
    }
}

/// `os.ReadFile` with Go's error messages (`open x: no such file or
/// directory`, `read x: is a directory`).
fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let name = path.to_string_lossy();
    let mut f = std::fs::File::open(path).map_err(|e| format!("open {name}: {}", GoIoError(&e)))?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)
        .map_err(|e| format!("read {name}: {}", GoIoError(&e)))?;
    Ok(data)
}

fn write_failure(e: io::Error) -> Failure {
    if e.kind() == io::ErrorKind::BrokenPipe {
        Failure::Exit(EXIT_BROKEN_PIPE)
    } else {
        Failure::Error(format!("write /dev/stdout: {}", GoIoError(&e)))
    }
}

/// Formats an OS error like Go's `syscall.Errno` (`no such file or
/// directory`) instead of Rust's `No such file or directory (os error 2)`.
struct GoIoError<'a>(&'a io::Error);

impl fmt::Display for GoIoError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.0.to_string();
        let text = match self.0.raw_os_error() {
            Some(code) => text
                .strip_suffix(&format!(" (os error {code})"))
                .unwrap_or(&text)
                .to_string(),
            None => text,
        };
        let mut chars = text.chars();
        match chars.next() {
            Some(c) => write!(f, "{}{}", c.to_lowercase(), chars.as_str()),
            None => Ok(()),
        }
    }
}

fn color_enabled(mode: &str, interactive: bool, stdout_is_terminal: bool) -> bool {
    match mode {
        "always" => true,
        "never" => false,
        _ => interactive || stdout_is_terminal,
    }
}
