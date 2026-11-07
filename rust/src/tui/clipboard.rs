//! Clipboard access: the system clipboard tool first, then the OSC 52
//! terminal escape sequence (which also works over SSH and inside
//! tmux/screen). Mirrors the Go version (atotto/clipboard + go-osc52).

use std::io::Write;
use std::process::{Command, Stdio};

/// Copies `text` to the clipboard.
pub fn copy(text: &str) -> Result<(), String> {
    if system_copy(text).is_ok() {
        return Ok(());
    }
    let seq = osc52(text, multiplexer());
    let mut err = std::io::stderr();
    err.write_all(seq.as_bytes())
        .and_then(|()| err.flush())
        .map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Multiplexer {
    None,
    Tmux,
    Screen,
}

fn multiplexer() -> Multiplexer {
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    if set("TMUX") {
        Multiplexer::Tmux
    } else if set("STY") {
        Multiplexer::Screen
    } else {
        Multiplexer::None
    }
}

/// The OSC 52 "set system clipboard" sequence, wrapped for tmux/screen like
/// go-osc52 does.
fn osc52(text: &str, mux: Multiplexer) -> String {
    let b64 = base64(text.as_bytes());
    match mux {
        Multiplexer::None => format!("\x1b]52;c;{b64}\x07"),
        Multiplexer::Tmux => format!("\x1bPtmux;\x1b\x1b]52;c;{b64}\x07\x1b\\"),
        Multiplexer::Screen => {
            let chunks: Vec<&str> = b64
                .as_bytes()
                .chunks(76)
                .map(|c| std::str::from_utf8(c).expect("base64 is ASCII"))
                .collect();
            format!("\x1bP\x1b]52;c;{}\x07\x1b\\", chunks.join("\x1b\\\x1bP"))
        }
    }
}

/// Candidate copy commands, in the order atotto/clipboard tries them.
fn copy_commands() -> Vec<(&'static str, &'static [&'static str])> {
    if cfg!(target_os = "macos") {
        return vec![("pbcopy", &[])];
    }
    if cfg!(windows) {
        return vec![("clip.exe", &[])];
    }
    let mut cmds: Vec<(&'static str, &'static [&'static str])> = Vec::new();
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()) {
        cmds.push(("wl-copy", &[]));
    }
    cmds.extend([
        ("xclip", &["-in", "-selection", "clipboard"][..]),
        ("xsel", &["--input", "--clipboard"][..]),
        ("termux-clipboard-set", &[][..]),
        ("clip.exe", &[][..]), // WSL
    ]);
    cmds
}

fn system_copy(text: &str) -> Result<(), String> {
    let payload: Vec<u8> = if cfg!(windows) {
        // clip.exe reads UTF-16LE when the input starts with a BOM.
        let mut v = vec![0xff, 0xfe];
        v.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        v
    } else {
        text.as_bytes().to_vec()
    };
    let mut last = String::from("no clipboard utility available");
    for (cmd, args) in copy_commands() {
        let child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let written = child
            .stdin
            .take()
            .map(|mut stdin| stdin.write_all(&payload))
            .unwrap_or(Ok(()));
        match (written, child.wait()) {
            (Ok(()), Ok(status)) if status.success() => return Ok(()),
            (Err(e), _) | (_, Err(e)) => last = e.to_string(),
            (_, Ok(status)) => last = format!("{cmd}: {status}"),
        }
    }
    Err(last)
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_std_encoding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64("日本".as_bytes()), "5pel5pys");
    }

    #[test]
    fn osc52_sequences_match_go_osc52() {
        assert_eq!(osc52("hi", Multiplexer::None), "\x1b]52;c;aGk=\x07");
        assert_eq!(
            osc52("hi", Multiplexer::Tmux),
            "\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\"
        );
        let long = "x".repeat(60); // 80 base64 chars -> two screen chunks
        let seq = osc52(&long, Multiplexer::Screen);
        assert!(seq.starts_with("\x1bP\x1b]52;c;"));
        assert_eq!(seq.matches("\x1b\\\x1bP").count(), 1);
        assert!(seq.ends_with("\x07\x1b\\"));
    }
}
