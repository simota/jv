//! Command-line parsing compatible with the Go version (cobra + pflag):
//! the same flag syntax (`-abc`, `-pVALUE`, `-p=VALUE`, `--flag=VALUE`,
//! `--flag VALUE`, interspersed positionals, `--`), value parsing
//! (`strconv.ParseBool`, `strconv.ParseInt(s, 0, 64)`) and error messages.

use std::ffi::OsString;

use crate::gostr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagKind {
    Bool,
    Str,
    Int,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagId {
    Interactive,
    NoInteractive,
    Type,
    Schema,
    Flat,
    Compact,
    Raw,
    SortKeys,
    Path,
    Diff,
    Depth,
    Theme,
    Color,
    Help,
}

struct FlagSpec {
    id: FlagId,
    name: &'static str,
    short: Option<u8>,
    kind: FlagKind,
}

const FLAGS: &[FlagSpec] = &[
    FlagSpec {
        id: FlagId::Interactive,
        name: "interactive",
        short: Some(b'i'),
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::NoInteractive,
        name: "no-interactive",
        short: Some(b'n'),
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Type,
        name: "type",
        short: Some(b't'),
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Schema,
        name: "schema",
        short: Some(b's'),
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Flat,
        name: "flat",
        short: Some(b'f'),
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Compact,
        name: "compact",
        short: None,
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Raw,
        name: "raw",
        short: Some(b'r'),
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::SortKeys,
        name: "sort-keys",
        short: None,
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Path,
        name: "path",
        short: Some(b'p'),
        kind: FlagKind::Str,
    },
    FlagSpec {
        id: FlagId::Diff,
        name: "diff",
        short: None,
        kind: FlagKind::Bool,
    },
    FlagSpec {
        id: FlagId::Depth,
        name: "depth",
        short: Some(b'd'),
        kind: FlagKind::Int,
    },
    FlagSpec {
        id: FlagId::Theme,
        name: "theme",
        short: None,
        kind: FlagKind::Str,
    },
    FlagSpec {
        id: FlagId::Color,
        name: "color",
        short: Some(b'c'),
        kind: FlagKind::Str,
    },
    FlagSpec {
        id: FlagId::Help,
        name: "help",
        short: Some(b'h'),
        kind: FlagKind::Bool,
    },
];

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

/// Parses `args` (without the program name). The error is the message that
/// follows `Error: `.
pub fn parse(args: Vec<OsString>) -> Result<Parsed, String> {
    let mut opts = Options::default();
    let mut positional = Vec::new();
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        let s = arg.to_string_lossy().into_owned();
        if s.len() < 2 || !s.starts_with('-') {
            positional.push(arg);
            continue;
        }
        if s.starts_with("--") {
            if s.len() == 2 {
                // `--` terminates flag parsing
                positional.extend(rest);
                break;
            }
            parse_long(&s, &mut rest, &mut opts)?;
        } else {
            parse_short(&s, &mut rest, &mut opts)?;
        }
    }
    Ok(Parsed {
        options: opts,
        args: positional,
    })
}

fn next_value(rest: &mut impl Iterator<Item = OsString>) -> Option<String> {
    rest.next().map(|v| v.to_string_lossy().into_owned())
}

fn parse_long(
    s: &str,
    rest: &mut impl Iterator<Item = OsString>,
    opts: &mut Options,
) -> Result<(), String> {
    let name = &s[2..];
    if name.starts_with(['-', '=']) {
        return Err(format!("bad flag syntax: {s}"));
    }
    let (name, inline) = match name.split_once('=') {
        Some((n, v)) => (n, Some(v.to_string())),
        None => (name, None),
    };
    let Some(spec) = FLAGS.iter().find(|f| f.name == name) else {
        return Err(format!("unknown flag: --{name}"));
    };
    let value = match inline {
        Some(v) => v,
        None if spec.kind == FlagKind::Bool => "true".into(),
        None => match next_value(rest) {
            Some(v) => v,
            None => return Err(format!("flag needs an argument: {s}")),
        },
    };
    set(spec, &value, opts)
}

fn parse_short(
    s: &str,
    rest: &mut impl Iterator<Item = OsString>,
    opts: &mut Options,
) -> Result<(), String> {
    let mut shorthands = &s[1..];
    while !shorthands.is_empty() {
        // pflag silently ignores Go test flags such as `-test.v`.
        if shorthands.starts_with("test.") {
            return Ok(());
        }
        let c = shorthands.as_bytes()[0];
        let Some(spec) = FLAGS.iter().find(|f| f.short == Some(c)) else {
            return Err(format!(
                "unknown shorthand flag: {} in -{shorthands}",
                gostr::quote_rune(char::from(c))
            ));
        };
        let after = &shorthands.as_bytes()[1..];
        let value;
        if after.len() > 1 && after[0] == b'=' {
            // `-f=arg`
            value = String::from_utf8_lossy(&after[1..]).into_owned();
            shorthands = "";
        } else if spec.kind == FlagKind::Bool {
            value = "true".into();
            // the flag character is ASCII, so this is a char boundary
            shorthands = &shorthands[1..];
        } else if !after.is_empty() {
            // `-farg`
            value = String::from_utf8_lossy(after).into_owned();
            shorthands = "";
        } else if let Some(v) = next_value(rest) {
            value = v;
            shorthands = "";
        } else {
            return Err(format!(
                "flag needs an argument: {} in -{shorthands}",
                gostr::quote_rune(char::from(c))
            ));
        }
        set(spec, &value, opts)?;
    }
    Ok(())
}

fn set(spec: &FlagSpec, value: &str, opts: &mut Options) -> Result<(), String> {
    let invalid = |err: String| {
        let flag_name = match spec.short {
            Some(c) => format!("-{}, --{}", char::from(c), spec.name),
            None => format!("--{}", spec.name),
        };
        format!(
            "invalid argument {} for {} flag: {err}",
            gostr::quote(value),
            gostr::quote(&flag_name)
        )
    };
    match spec.kind {
        FlagKind::Bool => {
            let b = parse_go_bool(value).ok_or_else(|| {
                invalid(format!(
                    "strconv.ParseBool: parsing {}: invalid syntax",
                    gostr::quote(value)
                ))
            })?;
            let field = match spec.id {
                FlagId::Interactive => &mut opts.interactive,
                FlagId::NoInteractive => &mut opts.no_interactive,
                FlagId::Type => &mut opts.show_type,
                FlagId::Schema => &mut opts.schema,
                FlagId::Flat => &mut opts.flat,
                FlagId::Compact => &mut opts.compact,
                FlagId::Raw => &mut opts.raw,
                FlagId::SortKeys => &mut opts.sort_keys,
                FlagId::Diff => &mut opts.diff,
                FlagId::Help => &mut opts.help,
                _ => unreachable!("not a bool flag"),
            };
            *field = b;
        }
        FlagKind::Int => {
            opts.depth = parse_go_int(value).map_err(invalid)?;
        }
        FlagKind::Str => match spec.id {
            FlagId::Path => opts.path = value.to_string(),
            FlagId::Theme => opts.theme = value.to_string(),
            FlagId::Color => {
                opts.color = value.to_string();
                opts.color_changed = true;
            }
            _ => unreachable!("not a string flag"),
        },
    }
    Ok(())
}

/// Go's `strconv.ParseBool`.
fn parse_go_bool(s: &str) -> Option<bool> {
    match s {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Some(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Some(false),
        _ => None,
    }
}

/// Go's `strconv.ParseInt(s, 0, 64)`: optional sign, `0x`/`0o`/`0b`/`0`
/// prefixes and `_` digit separators. The error text matches Go's.
fn parse_go_int(s0: &str) -> Result<i64, String> {
    let syntax = || {
        format!(
            "strconv.ParseInt: parsing {}: invalid syntax",
            gostr::quote(s0)
        )
    };
    let range = || {
        format!(
            "strconv.ParseInt: parsing {}: value out of range",
            gostr::quote(s0)
        )
    };
    if s0.is_empty() {
        return Err(syntax());
    }
    let (neg, unsigned) = match s0.as_bytes()[0] {
        b'+' => (false, &s0[1..]),
        b'-' => (true, &s0[1..]),
        _ => (false, s0),
    };
    if unsigned.is_empty() {
        return Err(syntax());
    }
    let b = unsigned.as_bytes();
    let lower = |c: u8| c | 0x20;
    let (base, digits): (u64, &[u8]) = if b[0] == b'0' {
        if b.len() >= 3 && lower(b[1]) == b'b' {
            (2, &b[2..])
        } else if b.len() >= 3 && lower(b[1]) == b'o' {
            (8, &b[2..])
        } else if b.len() >= 3 && lower(b[1]) == b'x' {
            (16, &b[2..])
        } else {
            (8, &b[1..])
        }
    } else {
        (10, b)
    };
    let mut n: u64 = 0;
    let mut underscores = false;
    for &c in digits {
        let d = match c {
            b'_' => {
                underscores = true;
                continue;
            }
            b'0'..=b'9' => c - b'0',
            _ if lower(c).is_ascii_lowercase() => lower(c) - b'a' + 10,
            _ => return Err(syntax()),
        };
        if u64::from(d) >= base {
            return Err(syntax());
        }
        // Go reports overflow as soon as it happens, before looking at the
        // remaining characters.
        n = n
            .checked_mul(base)
            .and_then(|v| v.checked_add(u64::from(d)))
            .ok_or_else(range)?;
    }
    if underscores && !underscore_ok(unsigned) {
        return Err(syntax());
    }
    const CUTOFF: u64 = 1 << 63;
    if (!neg && n >= CUTOFF) || (neg && n > CUTOFF) {
        return Err(range());
    }
    Ok(if neg {
        (n as i64).wrapping_neg()
    } else {
        n as i64
    })
}

/// Go's `underscoreOK`: underscores must separate digits.
fn underscore_ok(s: &str) -> bool {
    let b = s.as_bytes();
    let mut saw = b'^';
    let mut i = 0;
    let mut hex = false;
    let lower = |c: u8| c | 0x20;
    if b.len() >= 2 && b[0] == b'0' && matches!(lower(b[1]), b'b' | b'o' | b'x') {
        i = 2;
        saw = b'0';
        hex = lower(b[1]) == b'x';
    }
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() || (hex && (b'a'..=b'f').contains(&lower(c))) {
            saw = b'0';
        } else if c == b'_' {
            if saw != b'0' {
                return false;
            }
            saw = b'_';
        } else {
            if saw == b'_' {
                return false;
            }
            saw = b'!';
        }
        i += 1;
    }
    saw != b'_'
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

    #[test]
    fn go_int_parsing() {
        let ok = [
            ("0", 0),
            ("-3", -3),
            ("+7", 7),
            ("0x10", 16),
            ("0X1f", 31),
            ("0o17", 15),
            ("017", 15),
            ("0b101", 5),
            ("1_000", 1000),
            ("0x_1f", 31),
            ("-9223372036854775808", i64::MIN),
            ("9223372036854775807", i64::MAX),
        ];
        for (s, want) in ok {
            assert_eq!(parse_go_int(s), Ok(want), "{s}");
        }
        for s in [
            "", "-", "+", "08", "1__0", "_1", "1_", "0x", "1.5", " 1", "abc", "0b2",
        ] {
            assert!(
                parse_go_int(s).unwrap_err().ends_with("invalid syntax"),
                "{s}"
            );
        }
        for s in [
            "9223372036854775808",
            "-9223372036854775809",
            "0xffffffffffffffffff",
            "99999999999999999999x",
        ] {
            assert!(
                parse_go_int(s).unwrap_err().ends_with("value out of range"),
                "{s}"
            );
        }
    }
}
