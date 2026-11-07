//! Generates benchmark inputs: `cargo run --release --example gen_bench_data -- OUT_DIR`
//!
//! - `records.json`: an array of API-like records (~100 MB)
//! - `records.jsonl`: the same records as JSON Lines
//! - `wide.json`: one object with many keys

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "bench-data".into()),
    );
    std::fs::create_dir_all(&out).expect("create output directory");
    let n: usize = std::env::var("JV_BENCH_RECORDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(250_000);

    let mut seed: u64 = 42;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut records = Vec::with_capacity(n);
    for i in 0..n {
        let mut r = String::new();
        let r_ = next();
        write!(
            r,
            r#"{{"id":{i},"name":"user-{}","email":"user{}@example.com","active":{},"score":{}.{:02},"tags":["t{}","t{}","日本"],"address":{{"city":"City {}","zip":"{:05}","geo":[{}.5,-{}.25]}},"note":{}}}"#,
            r_ % 100_000,
            i,
            r_ % 2 == 0,
            r_ % 1000,
            r_ % 100,
            r_ % 7,
            r_ % 11,
            r_ % 50,
            r_ % 100_000,
            r_ % 90,
            r_ % 180,
            if r_ % 5 == 0 { "null".to_string() } else { format!("\"line\\nwith \\\"escapes\\\" {}\"", r_ % 997) },
        )
        .unwrap();
        records.push(r);
    }
    std::fs::write(
        out.join("records.json"),
        format!("[{}]", records.join(",\n")),
    )
    .unwrap();
    std::fs::write(out.join("records.jsonl"), records.join("\n")).unwrap();
    let wide: Vec<String> = (0..200_000).map(|i| format!("\"key{i}\":{i}")).collect();
    std::fs::write(out.join("wide.json"), format!("{{{}}}", wide.join(","))).unwrap();
    eprintln!("wrote {} records to {}", n, out.display());
}
