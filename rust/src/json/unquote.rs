//! Decoding of JSON string literals with the semantics of Go's
//! `json.Unmarshal` into a string: invalid UTF-8 bytes and unpaired
//! surrogate escapes become U+FFFD, and validation errors use Go's wording.

use crate::gostr::{decode_rune, json_quote_char};

/// Decodes a complete string literal (including both quotes).
/// Errors carry Go's message, e.g.
/// `invalid character 'x' in string escape code`.
pub(crate) fn unquote(lit: &[u8]) -> Result<String, String> {
    validate(lit)?;
    let body = &lit[1..lit.len() - 1];
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        let c = body[i];
        if c == b'\\' {
            let e = body[i + 1];
            i += 2;
            match e {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\x08'),
                b'f' => out.push('\x0c'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let r = hex4(&body[i..]).expect("validated \\u escape");
                    i += 4;
                    if (0xd800..0xe000).contains(&r) {
                        if (0xd800..0xdc00).contains(&r)
                            && let Some(r2) = escaped_u4(&body[i..])
                            && (0xdc00..0xe000).contains(&r2)
                        {
                            let combined = 0x10000 + ((r - 0xd800) << 10) + (r2 - 0xdc00);
                            out.push(char::from_u32(combined).unwrap_or('\u{fffd}'));
                            i += 6;
                            continue;
                        }
                        out.push('\u{fffd}');
                    } else {
                        out.push(char::from_u32(r).unwrap_or('\u{fffd}'));
                    }
                }
                _ => unreachable!("validated escape"),
            }
        } else if c < 0x80 {
            out.push(c as char);
            i += 1;
        } else {
            let (r, size) = decode_rune(&body[i..]);
            out.push(r);
            i += size;
        }
    }
    Ok(out)
}

/// Checks the literal like Go's `checkValid` scanner.
fn validate(lit: &[u8]) -> Result<(), String> {
    const EOF: &str = "unexpected end of JSON input";
    debug_assert_eq!(lit.first(), Some(&b'"'));
    let mut i = 1;
    while i < lit.len() {
        let c = lit[i];
        match c {
            b'"' => {
                return match lit.get(i + 1) {
                    None => Ok(()),
                    Some(&b) => Err(format!(
                        "invalid character {} after top-level value",
                        json_quote_char(b as char)
                    )),
                };
            }
            b'\\' => {
                let Some(&e) = lit.get(i + 1) else {
                    return Err(EOF.into());
                };
                match e {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => i += 2,
                    b'u' => {
                        for k in 0..4 {
                            match lit.get(i + 2 + k) {
                                None => return Err(EOF.into()),
                                Some(h) if h.is_ascii_hexdigit() => {}
                                Some(&h) => {
                                    return Err(format!(
                                        "invalid character {} in \\u hexadecimal character escape",
                                        json_quote_char(h as char)
                                    ));
                                }
                            }
                        }
                        i += 6;
                    }
                    // Go's quoteChar converts the *byte* to a rune.
                    _ => {
                        return Err(format!(
                            "invalid character {} in string escape code",
                            json_quote_char(e as char)
                        ));
                    }
                }
            }
            0..0x20 => {
                return Err(format!(
                    "invalid character {} in string literal",
                    json_quote_char(c as char)
                ));
            }
            _ => i += 1,
        }
    }
    Err(EOF.into())
}

fn hex4(b: &[u8]) -> Option<u32> {
    let s = std::str::from_utf8(b.get(..4)?).ok()?;
    u32::from_str_radix(s, 16).ok()
}

/// `\uXXXX` at the start of `b`.
fn escaped_u4(b: &[u8]) -> Option<u32> {
    if b.len() >= 6 && b[0] == b'\\' && b[1] == b'u' {
        hex4(&b[2..6])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::unquote;

    #[test]
    fn escapes_and_surrogates() {
        assert_eq!(unquote(br#""a\n\t\/\"\\""#).unwrap(), "a\n\t/\"\\");
        assert_eq!(unquote(b"\"\\u00e9\\ud83d\\ude00\"").unwrap(), "é😀");
        assert_eq!(unquote(br#""\ud800x\udc00""#).unwrap(), "\u{fffd}x\u{fffd}");
        // high surrogate followed by a non-low escape: replacement, then the escape
        assert_eq!(unquote(br#""\ud800A""#).unwrap(), "\u{fffd}A");
        assert_eq!(
            unquote(b"\"\xff\xe2\x82A\"").unwrap(),
            "\u{fffd}\u{fffd}\u{fffd}A"
        );
    }

    #[test]
    fn errors_use_go_wording() {
        assert_eq!(
            unquote(br#""\x""#).unwrap_err(),
            "invalid character 'x' in string escape code"
        );
        assert_eq!(
            unquote(br#""\u12g4""#).unwrap_err(),
            "invalid character 'g' in \\u hexadecimal character escape"
        );
        assert_eq!(
            unquote(b"\"\\\xc3\xa9\"").unwrap_err(),
            "invalid character '\u{c3}' in string escape code"
        );
        assert_eq!(
            unquote(b"\"a\tb\"").unwrap_err(),
            "invalid character '\\t' in string literal"
        );
        assert_eq!(
            unquote(br#""abc"#).unwrap_err(),
            "unexpected end of JSON input"
        );
    }
}
