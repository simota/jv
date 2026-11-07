//! Helpers that reproduce Go's `strconv` quoting rules.
//!
//! jv's error messages are part of its observable behaviour (scripts and
//! tests match on them), and the Go implementation builds them with
//! `strconv.Quote` / `%q`. These helpers produce the same text.

/// Go's `strconv.Quote(s)`: a double-quoted string with Go escapes.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        push_escaped(&mut out, c, '"');
    }
    out.push('"');
    out
}

/// Go's `strconv.QuoteRune(r)` (also what `%q` prints for a rune or byte).
pub fn quote_rune(c: char) -> String {
    let mut out = String::from("'");
    push_escaped(&mut out, c, '\'');
    out.push('\'');
    out
}

/// The `quoteChar` helper used by Go's `encoding/json` scanner:
/// `'x'` with the inner text of `strconv.Quote`, so `'` becomes `'\''` and
/// `"` stays `'"'`.
pub fn json_quote_char(c: char) -> String {
    match c {
        '\'' => r"'\''".to_string(),
        '"' => "'\"'".to_string(),
        _ => {
            let mut out = String::from("'");
            push_escaped(&mut out, c, '"');
            out.push('\'');
            out
        }
    }
}

fn push_escaped(out: &mut String, c: char, quote: char) {
    if c == quote || c == '\\' {
        out.push('\\');
        out.push(c);
        return;
    }
    if is_print(c) {
        out.push(c);
        return;
    }
    match c {
        '\x07' => out.push_str(r"\a"),
        '\x08' => out.push_str(r"\b"),
        '\x0c' => out.push_str(r"\f"),
        '\n' => out.push_str(r"\n"),
        '\r' => out.push_str(r"\r"),
        '\t' => out.push_str(r"\t"),
        '\x0b' => out.push_str(r"\v"),
        c if (c as u32) < 0x20 || c == '\x7f' => {
            out.push_str(&format!(r"\x{:02x}", c as u32));
        }
        c if (c as u32) < 0x10000 => out.push_str(&format!(r"\u{:04x}", c as u32)),
        c => out.push_str(&format!(r"\U{:08x}", c as u32)),
    }
}

/// Approximation of Go's `strconv.IsPrint`: letters, marks, numbers,
/// punctuation, symbols and the ASCII space. Control, format, separator
/// (other than U+0020) and private-use characters are not printable.
/// Unassigned code points are treated as printable (Go treats them as not
/// printable); this only affects how exotic characters appear inside error
/// messages.
pub fn is_print(c: char) -> bool {
    let u = c as u32;
    if u < 0x20 || (0x7f..=0x9f).contains(&u) {
        return false;
    }
    !matches!(u,
        0xad
        | 0xa0
        | 0x600..=0x605
        | 0x61c
        | 0x6dd
        | 0x70f
        | 0x1680
        | 0x180e
        | 0x2000..=0x200f
        | 0x2028..=0x202f
        | 0x205f..=0x2064
        | 0x2066..=0x206f
        | 0x3000
        | 0xd800..=0xdfff
        | 0xe000..=0xf8ff
        | 0xfeff
        | 0xfff9..=0xfffb
        | 0x110bd
        | 0x1d173..=0x1d17a
        | 0xe0001
        | 0xe0020..=0xe007f
        | 0xf0000..
    )
}

/// Decodes the first UTF-8 character of `b` the way Go's
/// `utf8.DecodeRune` does: invalid or truncated sequences yield U+FFFD with
/// a size of 1.
pub fn decode_rune(b: &[u8]) -> (char, usize) {
    if b.is_empty() {
        return ('\u{fffd}', 0);
    }
    let n = utf8_len(b[0]);
    if n > 0
        && b.len() >= n
        && let Ok(s) = std::str::from_utf8(&b[..n])
        && let Some(c) = s.chars().next()
    {
        return (c, n);
    }
    ('\u{fffd}', 1)
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

/// Converts bytes to a string replacing every byte of an invalid UTF-8
/// sequence with U+FFFD (Go's behaviour), rather than one replacement per
/// maximal invalid subpart (Rust's `from_utf8_lossy`).
pub fn bytes_to_string_go(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let mut out = String::with_capacity(b.len() + 8);
            for chunk in b.utf8_chunks() {
                out.push_str(chunk.valid());
                for _ in chunk.invalid() {
                    out.push('\u{fffd}');
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_matches_go() {
        assert_eq!(quote("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(quote("tab\there\n"), r#""tab\there\n""#);
        assert_eq!(quote("\x01\x7f"), r#""\x01\x7f""#);
        assert_eq!(quote("日本 é"), "\"日本 é\"");
        assert_eq!(quote("\u{a0}\u{feff}"), r#""\u00a0\ufeff""#);
        assert_eq!(quote("\u{f0000}"), r#""\U000f0000""#);
    }

    #[test]
    fn quote_rune_and_json_quote_char() {
        assert_eq!(quote_rune('x'), "'x'");
        assert_eq!(quote_rune('\''), r"'\''");
        assert_eq!(quote_rune('"'), "'\"'");
        assert_eq!(json_quote_char('\''), r"'\''");
        assert_eq!(json_quote_char('"'), "'\"'");
        assert_eq!(json_quote_char('\x01'), r"'\x01'");
        assert_eq!(json_quote_char('\u{3000}'), r"'\u3000'");
        assert_eq!(json_quote_char('\u{fffd}'), "'\u{fffd}'");
    }

    #[test]
    fn decode_rune_like_go() {
        assert_eq!(decode_rune(b"a"), ('a', 1));
        assert_eq!(decode_rune("é!".as_bytes()), ('é', 2));
        assert_eq!(decode_rune(b"\xff"), ('\u{fffd}', 1));
        assert_eq!(decode_rune(b"\xe2\x82"), ('\u{fffd}', 1));
        assert_eq!(decode_rune(b"\xed\xa0\x80"), ('\u{fffd}', 1)); // surrogate
    }

    #[test]
    fn replacement_is_per_byte() {
        assert_eq!(
            bytes_to_string_go(b"\xff\xe2\x82A"),
            "\u{fffd}\u{fffd}\u{fffd}A"
        );
        assert_eq!(
            bytes_to_string_go(b"\xed\xa0\x80"),
            "\u{fffd}\u{fffd}\u{fffd}"
        );
        assert_eq!(bytes_to_string_go("ok".as_bytes()), "ok");
    }
}
