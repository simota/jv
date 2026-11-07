//! The `jv` binary.

/// Stack size for the worker thread. Parsing is iterative, but formatting
/// and schema inference recurse once per nesting level (up to 10000
/// levels), which needs more than the default main-thread stack on some
/// platforms. The memory is reserved virtually and only committed on use.
const STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() {
    let code = std::thread::Builder::new()
        .name("jv".into())
        .stack_size(STACK_SIZE)
        .spawn(jv::cli::main)
        .expect("failed to start jv worker thread")
        .join()
        .unwrap_or(2);
    std::process::exit(code);
}
