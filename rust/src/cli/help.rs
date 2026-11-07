//! The text cobra prints for `jv --help` and on usage errors, kept
//! byte-for-byte identical to the Go version.

/// The command's long description; `--help` prints it, a blank line and
/// [`USAGE`].
pub const LONG: &str = r##"JSON viewer for the terminal.

Reads a JSON document (or JSON Lines / several concatenated documents) from
FILE or stdin ("-" also means stdin) and pretty prints it, or opens an
interactive collapsible viewer with -i.

Paths use the same syntax that the viewer shows in its footer:
  $.users[0].name   .users[-1]   users[*].email   $["key with space"]"##;

/// The usage block. Flag errors print it to stdout followed by a blank line.
pub const USAGE: &str = r##"Usage:
  jv [OPTIONS] [FILE]
  jv --diff OLD NEW [flags]

Examples:
  jv data.json                      # pretty print
  jv -p '.items[*].id' data.json    # extract values by path
  jv -r -p .name data.json          # raw string output for scripts
  jv -f data.json | grep email      # greppable "path = value" lines
  jv -s data.json                   # inferred schema
  jv -i data.json                   # interactive viewer
  jv --diff old.json new.json       # structural diff (exit 1 if different)
  kubectl get pods -o json | jv -i -p '.items[*].status.phase'

Flags:
  -c, --color string     Color (auto/always/never); NO_COLOR is honored when not set (default "always")
      --compact          Print each document on a single line
  -d, --depth int        Initial expand depth (interactive) (default 2)
      --diff             Compare two JSON inputs and list differences by path
  -f, --flat             Print one 'path = value' line per leaf (greppable)
  -h, --help             help for jv
  -i, --interactive      Force interactive mode
  -n, --no-interactive   Force non-interactive mode
  -p, --path string      Select values by path, e.g. '.items[*].name' (interactive: jump to them)
  -r, --raw              Print string results without quotes
  -s, --schema           Show inferred schema (merges all array elements)
      --sort-keys        Sort object keys alphabetically (default: input order)
      --theme string     Theme (dark/light) (default "dark")
  -t, --type             Show type hints
"##;
