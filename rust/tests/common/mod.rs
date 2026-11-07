//! Shared helpers for integration tests: a small deterministic PRNG and
//! generators of JSON text (valid and corrupted).
#![allow(dead_code)]

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

// --- random JSON ---------------------------------------------------------

pub const KEYS: &[&str] = &[
    "a",
    "b",
    "id",
    "name",
    "_x",
    "k1",
    "kebab-key",
    "with space",
    "a.b",
    "",
    "日本",
    "é",
    "q\"uote",
    "back\\slash",
    "tab\tkey",
    "Z",
    "A9",
    "9a",
    "$",
    "[x]",
    "line\u{2028}sep",
];

pub fn gen_string(rng: &mut Rng) -> String {
    let parts = [
        "plain",
        "",
        " ",
        "Tokyo",
        "x<y&z>",
        "日本語",
        "😀",
        "\\n",
        "\\t",
        "\\\"",
        "\\\\",
        "\\/",
        "\\b",
        "\\f",
        "\\r",
        "\\u0000",
        "\\u001f",
        "\\u007f",
        "\\u00e9",
        "\\u2028",
        "\\u2029",
        "\\ud83d\\ude00",
        "\\ud800",
        "\\udc00x",
        "\\ud800\\u0041",
        "{\\\"inner\\\": [1, 2]}",
        "\u{7f}",
        "needle",
        "NEEDLE",
    ];
    let n = rng.below(4);
    let mut s = String::from("\"");
    for _ in 0..n {
        s.push_str(rng.pick(&parts));
    }
    s.push('"');
    s
}

pub fn gen_number(rng: &mut Rng) -> String {
    let forms = [
        "0",
        "-0",
        "1",
        "-1",
        "42",
        "1.0",
        "1.50",
        "0.5",
        "-0.25",
        "1e3",
        "1E3",
        "1e+3",
        "1e-3",
        "10e-1",
        "123456789",
        "-987654321.125",
        "3.14159",
        "2.5e10",
        "0.0",
        "1200",
        "1.2e3",
        "100",
        "1e2",
    ];
    rng.pick(&forms).to_string()
}

pub fn ws(rng: &mut Rng) -> &'static str {
    rng.pick(&["", "", "", " ", "\n", "  ", "\t", "\r\n"])
}

pub fn gen_value(rng: &mut Rng, depth: usize) -> String {
    let choice = if depth >= 4 {
        rng.below(5)
    } else {
        rng.below(8)
    };
    match choice {
        0 => gen_string(rng),
        1 => gen_number(rng),
        2 => rng.pick(&["true", "false"]).to_string(),
        3 => "null".into(),
        4 => gen_string(rng),
        5 | 6 => {
            let n = rng.below(5);
            let members: Vec<String> = (0..n)
                .map(|_| {
                    let key = rng.pick(KEYS);
                    let key = serde_like_quote(key);
                    format!(
                        "{}{key}{}:{}{}",
                        ws(rng),
                        ws(rng),
                        ws(rng),
                        gen_value(rng, depth + 1)
                    )
                })
                .collect();
            format!("{{{}{}}}", members.join(","), ws(rng))
        }
        _ => {
            let n = rng.below(5);
            let items: Vec<String> = (0..n)
                .map(|_| format!("{}{}", ws(rng), gen_value(rng, depth + 1)))
                .collect();
            format!("[{}{}]", items.join(","), ws(rng))
        }
    }
}

/// JSON-quotes a key (keys in KEYS are plain text).
pub fn serde_like_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn gen_document(rng: &mut Rng) -> String {
    if rng.chance(15) {
        // JSON Lines / concatenated documents
        let n = 1 + rng.below(3);
        let docs: Vec<String> = (0..n).map(|_| gen_value(rng, 1)).collect();
        let sep = rng.pick(&["\n", " ", "", "\n\n"]);
        let mut out = docs[0].clone();
        for d in &docs[1..] {
            // `1` `2` must not be glued into `12`; only delimiters may touch
            let glued_ok = out.ends_with(['}', ']', '"']) || d.starts_with(['{', '[', '"']);
            out.push_str(if sep.is_empty() && !glued_ok {
                "\n"
            } else {
                sep
            });
            out.push_str(d);
        }
        out
    } else {
        format!("{}{}{}", ws(rng), gen_value(rng, 0), ws(rng))
    }
}

pub fn mutate(rng: &mut Rng, s: &str) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    let edits = 1 + rng.below(3);
    for _ in 0..edits {
        let pos = if b.is_empty() {
            0
        } else {
            rng.below(b.len() + 1)
        };
        let insert: &[u8] = rng.pick(&[
            &b"{"[..],
            b"}",
            b"[",
            b"]",
            b",",
            b":",
            b"\"",
            b"\\",
            b"x",
            b"1",
            b"-",
            b".",
            b"e",
            b" ",
            b"\n",
            b"t",
            b"n",
            b"\x01",
            b"\xff",
            b"\xc3\xa9",
            b"\xe3\x80\x80",
            b"'",
            b"/",
            b"0",
            b"\\u12",
            b"\\q",
            b"\xc2\xa0",
            b"\x7f",
        ]);
        match rng.below(3) {
            0 if !b.is_empty() && pos < b.len() => {
                b.remove(pos);
            }
            1 if pos < b.len() => {
                b.truncate(pos);
            }
            _ => {
                b.splice(pos..pos, insert.iter().copied());
            }
        }
    }
    b
}
