//! jv: a JSON viewer for the terminal.
//!
//! The library exposes the building blocks used by the `jv` binary:
//! an order-preserving JSON [parser](json), [path queries](path),
//! [structural diffs](diff), output [formatters](format), the
//! [command line](cli) and the interactive [viewer](tui).

pub mod cli;
pub mod diff;
pub mod format;
pub mod gostr;
pub mod json;
pub mod path;
pub mod tui;
