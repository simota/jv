//! A pflag-compatible flag parser: the same syntax (`-abc`, `-pVALUE`,
//! `-p=VALUE`, `--flag=VALUE`, `--flag VALUE`, interspersed positionals,
//! `--`), value parsing (`strconv.ParseBool`, `strconv.ParseInt(s, 0, 64)`)
//! and error messages as the Go version of jv (cobra + pflag).

use std::ffi::OsString;

use crate::gostr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagKind {
    Bool,
    Str,
    Int,
}

/// One flag definition.
#[derive(Clone, Copy, Debug)]
pub struct FlagSpec {
    pub name: &'static str,
    pub short: Option<u8>,
    pub kind: FlagKind,
    pub usage: &'static str,
}

/// A parsed flag value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
}

/// Result of parsing a command line against a set of flags.
#[derive(Clone, Debug, Default)]
pub struct Parsed {
    /// The last value given for each flag (indexed like the specs).
    pub values: Vec<Option<Value>>,
    /// Positional arguments, including everything after `--`.
    pub args: Vec<OsString>,
}

impl Parsed {
    pub fn changed(&self, i: usize) -> bool {
        self.values[i].is_some()
    }
}

/// Parses `args` against `specs`. The error is the message that follows
/// `Error: `.
pub fn parse(specs: &[FlagSpec], args: Vec<OsString>) -> Result<Parsed, String> {
    let mut parsed = Parsed {
        values: vec![None; specs.len()],
        args: Vec::new(),
    };
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        let s = arg.to_string_lossy().into_owned();
        if s.len() < 2 || !s.starts_with('-') {
            parsed.args.push(arg);
            continue;
        }
        if s.starts_with("--") {
            if s.len() == 2 {
                // `--` terminates flag parsing
                parsed.args.extend(rest);
                break;
            }
            parse_long(specs, &s, &mut rest, &mut parsed)?;
        } else {
            parse_short(specs, &s, &mut rest, &mut parsed)?;
        }
    }
    Ok(parsed)
}

fn next_value(rest: &mut impl Iterator<Item = OsString>) -> Option<String> {
    rest.next().map(|v| v.to_string_lossy().into_owned())
}

fn parse_long(
    specs: &[FlagSpec],
    s: &str,
    rest: &mut impl Iterator<Item = OsString>,
    parsed: &mut Parsed,
) -> Result<(), String> {
    let name = &s[2..];
    if name.starts_with(['-', '=']) {
        return Err(format!("bad flag syntax: {s}"));
    }
    let (name, inline) = match name.split_once('=') {
        Some((n, v)) => (n, Some(v.to_string())),
        None => (name, None),
    };
    let Some(i) = specs.iter().position(|f| f.name == name) else {
        return Err(format!("unknown flag: --{name}"));
    };
    let value = match inline {
        Some(v) => v,
        None if specs[i].kind == FlagKind::Bool => "true".into(),
        None => match next_value(rest) {
            Some(v) => v,
            None => return Err(format!("flag needs an argument: {s}")),
        },
    };
    set(specs, i, &value, parsed)
}

fn parse_short(
    specs: &[FlagSpec],
    s: &str,
    rest: &mut impl Iterator<Item = OsString>,
    parsed: &mut Parsed,
) -> Result<(), String> {
    let mut shorthands = &s[1..];
    while !shorthands.is_empty() {
        // pflag silently ignores Go test flags such as `-test.v`.
        if shorthands.starts_with("test.") {
            return Ok(());
        }
        let c = shorthands.as_bytes()[0];
        let Some(i) = specs.iter().position(|f| f.short == Some(c)) else {
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
        } else if specs[i].kind == FlagKind::Bool {
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
        set(specs, i, &value, parsed)?;
    }
    Ok(())
}

fn set(specs: &[FlagSpec], i: usize, value: &str, parsed: &mut Parsed) -> Result<(), String> {
    let spec = &specs[i];
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
    let v = match spec.kind {
        FlagKind::Bool => Value::Bool(parse_go_bool(value).ok_or_else(|| {
            invalid(format!(
                "strconv.ParseBool: parsing {}: invalid syntax",
                gostr::quote(value)
            ))
        })?),
        FlagKind::Int => Value::Int(parse_go_int(value).map_err(invalid)?),
        FlagKind::Str => Value::Str(value.to_string()),
    };
    parsed.values[i] = Some(v);
    Ok(())
}

/// Go's `strconv.ParseBool`.
pub fn parse_go_bool(s: &str) -> Option<bool> {
    match s {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Some(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Some(false),
        _ => None,
    }
}

/// Go's `strconv.ParseInt(s, 0, 64)`: optional sign, `0x`/`0o`/`0b`/`0`
/// prefixes and `_` digit separators. The error text matches Go's.
pub fn parse_go_int(s0: &str) -> Result<i64, String> {
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
