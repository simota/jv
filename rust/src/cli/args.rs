//! The root command's flags and their mapping to [`Options`].

use std::ffi::OsString;

use super::flags::{self, FlagKind, FlagSpec, Value};

/// Root flags in pflag's (alphabetical) order, as shown by `--help` and
/// shell completion.
pub(crate) const FLAGS: &[FlagSpec] = &[
    spec(
        "color",
        Some(b'c'),
        FlagKind::Str,
        "Color (auto/always/never); NO_COLOR is honored when not set",
    ),
    spec(
        "compact",
        None,
        FlagKind::Bool,
        "Print each document on a single line",
    ),
    spec(
        "depth",
        Some(b'd'),
        FlagKind::Int,
        "Initial expand depth (interactive)",
    ),
    spec(
        "diff",
        None,
        FlagKind::Bool,
        "Compare two JSON inputs and list differences by path",
    ),
    spec(
        "flat",
        Some(b'f'),
        FlagKind::Bool,
        "Print one 'path = value' line per leaf (greppable)",
    ),
    spec("help", Some(b'h'), FlagKind::Bool, "help for jv"),
    spec(
        "interactive",
        Some(b'i'),
        FlagKind::Bool,
        "Force interactive mode",
    ),
    spec(
        "no-interactive",
        Some(b'n'),
        FlagKind::Bool,
        "Force non-interactive mode",
    ),
    spec(
        "path",
        Some(b'p'),
        FlagKind::Str,
        "Select values by path, e.g. '.items[*].name' (interactive: jump to them)",
    ),
    spec(
        "raw",
        Some(b'r'),
        FlagKind::Bool,
        "Print string results without quotes",
    ),
    spec(
        "schema",
        Some(b's'),
        FlagKind::Bool,
        "Show inferred schema (merges all array elements)",
    ),
    spec(
        "sort-keys",
        None,
        FlagKind::Bool,
        "Sort object keys alphabetically (default: input order)",
    ),
    spec("theme", None, FlagKind::Str, "Theme (dark/light)"),
    spec("type", Some(b't'), FlagKind::Bool, "Show type hints"),
];

const fn spec(
    name: &'static str,
    short: Option<u8>,
    kind: FlagKind,
    usage: &'static str,
) -> FlagSpec {
    FlagSpec {
        name,
        short,
        kind,
        usage,
    }
}

/// Parsed command-line options (defaults match the Go flags).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub interactive: bool,
    pub no_interactive: bool,
    pub show_type: bool,
    pub schema: bool,
    pub flat: bool,
    pub compact: bool,
    pub raw: bool,
    pub sort_keys: bool,
    pub diff: bool,
    pub path: String,
    pub depth: i64,
    pub theme: String,
    pub color: String,
    /// `--color`/`-c` was given explicitly (then `NO_COLOR` is ignored).
    pub color_changed: bool,
    pub help: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            interactive: false,
            no_interactive: false,
            show_type: false,
            schema: false,
            flat: false,
            compact: false,
            raw: false,
            sort_keys: false,
            diff: false,
            path: String::new(),
            depth: 2,
            theme: "dark".into(),
            color: "always".into(),
            color_changed: false,
            help: false,
        }
    }
}

/// The result of parsing: options plus positional arguments (kept as
/// `OsString` so that non-UTF-8 file names work).
#[derive(Debug)]
pub struct Parsed {
    pub options: Options,
    pub args: Vec<OsString>,
}

/// Parses the root command's `args` (without the program name). The error
/// is the message that follows `Error: `.
pub fn parse(args: Vec<OsString>) -> Result<Parsed, String> {
    let parsed = flags::parse(FLAGS, args)?;
    let mut o = Options::default();
    for (spec, value) in FLAGS.iter().zip(parsed.values) {
        let Some(value) = value else { continue };
        match (spec.name, value) {
            ("interactive", Value::Bool(b)) => o.interactive = b,
            ("no-interactive", Value::Bool(b)) => o.no_interactive = b,
            ("type", Value::Bool(b)) => o.show_type = b,
            ("schema", Value::Bool(b)) => o.schema = b,
            ("flat", Value::Bool(b)) => o.flat = b,
            ("compact", Value::Bool(b)) => o.compact = b,
            ("raw", Value::Bool(b)) => o.raw = b,
            ("sort-keys", Value::Bool(b)) => o.sort_keys = b,
            ("diff", Value::Bool(b)) => o.diff = b,
            ("help", Value::Bool(b)) => o.help = b,
            ("depth", Value::Int(n)) => o.depth = n,
            ("path", Value::Str(s)) => o.path = s,
            ("theme", Value::Str(s)) => o.theme = s,
            ("color", Value::Str(s)) => {
                o.color = s;
                o.color_changed = true;
            }
            (name, v) => unreachable!("flag {name} has an unexpected value {v:?}"),
        }
    }
    Ok(Parsed {
        options: o,
        args: parsed.args,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Parsed, String> {
        parse(args.iter().map(OsString::from).collect())
    }

    #[test]
    fn flag_forms() {
        let r = p(&[
            "-rt",
            "-p.name",
            "file",
            "--color=never",
            "-d",
            "3",
            "--theme",
            "light",
        ])
        .unwrap();
        let o = &r.options;
        assert!(o.raw && o.show_type);
        assert_eq!(o.path, ".name");
        assert_eq!(o.color, "never");
        assert!(o.color_changed);
        assert_eq!(o.depth, 3);
        assert_eq!(o.theme, "light");
        assert_eq!(r.args, ["file"]);

        let r = p(&["-p=.a", "-r=false", "-", "--", "-x", "--y"]).unwrap();
        assert_eq!(r.options.path, ".a");
        assert!(!r.options.raw);
        assert_eq!(r.args, ["-", "-x", "--y"]);

        // a value-taking flag consumes the next argument even if it looks like a flag
        let r = p(&["-p", "-r", "--path", "--x"]).unwrap();
        assert_eq!(r.options.path, "--x");
        assert!(!r.options.raw);

        assert!(p(&["-h"]).unwrap().options.help);
        assert!(!p(&["--help=false"]).unwrap().options.help);
        assert_eq!(p(&["-test.v", "x"]).unwrap().args, ["x"]);
    }

    #[test]
    fn errors_match_pflag() {
        let cases: &[(&[&str], &str)] = &[
            (&["--bogus"], "unknown flag: --bogus"),
            (&["--bogus=1"], "unknown flag: --bogus"),
            (&["-x"], "unknown shorthand flag: 'x' in -x"),
            (&["-rx"], "unknown shorthand flag: 'x' in -x"),
            (&["-xr"], "unknown shorthand flag: 'x' in -xr"),
            (&["-r="], "unknown shorthand flag: '=' in -="),
            (&["-p"], "flag needs an argument: 'p' in -p"),
            (&["-rp"], "flag needs an argument: 'p' in -p"),
            (&["--path"], "flag needs an argument: --path"),
            (&["---x"], "bad flag syntax: ---x"),
            (&["--=x"], "bad flag syntax: --=x"),
            (
                &["-d", "abc"],
                r#"invalid argument "abc" for "-d, --depth" flag: strconv.ParseInt: parsing "abc": invalid syntax"#,
            ),
            (
                &["--compact=yes"],
                r#"invalid argument "yes" for "--compact" flag: strconv.ParseBool: parsing "yes": invalid syntax"#,
            ),
            (
                &["-d", "99999999999999999999"],
                r#"invalid argument "99999999999999999999" for "-d, --depth" flag: strconv.ParseInt: parsing "99999999999999999999": value out of range"#,
            ),
        ];
        for (args, want) in cases {
            assert_eq!(p(args).unwrap_err(), *want, "args {args:?}");
        }
    }
}
