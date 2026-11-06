# jv

A simple JSON viewer written in Go. Supports pretty formatting, path extraction, schema inference, structural diffs, and a collapsible interactive TUI.

## Features
- Pretty printing that keeps the original key order (`--sort-keys` to sort)
- JSON Lines / multiple concatenated documents
- Path extraction with `-p` (`.items[*].name`, `.list[-1]`, `["odd key"]`), raw output with `-r`, single-line output with `--compact`
- Greppable `path = value` output (`-f`)
- Schema inference that merges all array elements and documents (optional keys, union types)
- Structural diff of two documents (`--diff`), with exit code 1 when they differ
- Syntax errors report line and column
- Type hints (use `-t` in pipe mode)
- Collapsible interactive TUI that stays responsive on large files
- TUI search across the whole document (collapsed parts included), path jumps, `n`/`N` match navigation
- Copy the selected value or path; full-value view with embedded-JSON pretty printing

## Install

### Go install

```bash
go install github.com/simota/jv/cmd/jv@latest
```

### From GitHub Releases

Download a prebuilt binary from the Releases page and put it in your `PATH`.

Examples:

```bash
# macOS (arm64)
curl -LO https://github.com/simota/jv/releases/download/v0.1.0/jv_v0.1.0_darwin_arm64.tar.gz
tar -xzf jv_v0.1.0_darwin_arm64.tar.gz
sudo mv jv_v0.1.0_darwin_arm64 /usr/local/bin/jv

# Linux (amd64)
curl -LO https://github.com/simota/jv/releases/download/v0.1.0/jv_v0.1.0_linux_amd64.tar.gz
tar -xzf jv_v0.1.0_linux_amd64.tar.gz
sudo mv jv_v0.1.0_linux_amd64 /usr/local/bin/jv

# Windows (amd64) - PowerShell
Invoke-WebRequest -Uri https://github.com/simota/jv/releases/download/v0.1.0/jv_v0.1.0_windows_amd64.zip -OutFile jv_v0.1.0_windows_amd64.zip
Expand-Archive jv_v0.1.0_windows_amd64.zip -DestinationPath .
Move-Item jv_v0.1.0_windows_amd64.exe C:\\Windows\\System32\\jv.exe
```

## Build

```bash
make build
```

This produces `./jv`.

## Release

Release workflow and local dry-run steps are documented in `docs/RELEASING.md`.

## Usage

### Pipe mode (default)

```bash
jv file.json
cat file.json | jv
cat file.json | jv -        # "-" also reads stdin
```

Input can be a single JSON document, JSON Lines, or several concatenated
documents. Each document is printed in turn.

With type hints:

```bash
cat file.json | jv -t
```

Schema view (array elements and JSON Lines records are merged; `?` marks keys
that are missing in some elements, `|` lists every observed type):

```bash
cat file.json | jv -s
```

```
{
  "projects": [
    {
      "id": number,
      "status": string | null,
      "owner"?: string
    }
  ]
}
```

### Extracting values by path

Paths use the same syntax the TUI shows in its footer (and copies with `Y`).
The leading `$` is optional.

```bash
jv -p '.address.city' file.json          # "Tokyo"
jv -r -p '.address.city' file.json       # Tokyo   (raw string, for scripts)
jv -p '.projects[*].name' file.json      # every project name, one per line
jv -p '.projects[-1]' file.json          # last element
jv -p '$.misc["key with space"]' file.json
jv --compact -p .projects file.json      # single-line JSON
```

If the path matches nothing, `jv` prints an error and exits with status 1.

### Greppable output

```bash
jv -f file.json
# $.name = "John Doe"
# $.address.city = "Tokyo"
# $.projects[0].id = 1
jv -f file.json | grep status     # find where a field lives, then use the path with -p
```

### Comparing two documents

```bash
jv --diff old.json new.json
# ~ $.age: 30 -> 31
# + $.tags[3]: "new"
# - $.legacy: {"a":1}
curl -s https://api.example.com/config | jv --diff saved.json -
jv --diff -p .spec old.json new.json  # compare only a subtree
```

Objects are compared by key (order is ignored), arrays by index, and numbers
by value (`1` equals `1.0`). The exit status is 0 when the documents are equal
and 1 when they differ, like `diff`.

### Interactive mode (TUI)

```bash
jv -i file.json
cat file.json | jv -i
jv -i -p '.items[*].status' file.json   # open with the cursor on the matches (n/N to cycle)
```

You can enable type hints initially with `-t`, and toggle them in TUI with `t`.

## Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--interactive` | `-i` | Force interactive mode | false |
| `--no-interactive` | `-n` | Force pipe mode | false |
| `--path` | `-p` | Select values by path (TUI: jump to them) | |
| `--raw` | `-r` | Print string results without quotes | false |
| `--compact` |  | One line per document | false |
| `--flat` | `-f` | Print `path = value` lines | false |
| `--type` | `-t` | Show type hints | false |
| `--schema` | `-s` | Schema mode | false |
| `--sort-keys` |  | Sort object keys alphabetically | false |
| `--diff` |  | Compare two inputs (`jv --diff OLD NEW`) | false |
| `--depth` | `-d` | Initial expand depth (TUI) | 2 |
| `--theme` |  | Theme (dark/light) | dark |
| `--color` | `-c` | Color (auto/always/never) | always |

`NO_COLOR` is honored when `--color` is not given explicitly.

## TUI key bindings

| Key | Action |
|-----|--------|
| `↑`/`k` | Move up |
| `↓`/`j` | Move down |
| `PgUp`/`PgDn` | Page up/down |
| `Ctrl+u`/`Ctrl+d` | Page up/down |
| `←`/`h` | Collapse (or go to parent) |
| `→`/`l` | Expand |
| `Space` | Toggle |
| `Enter` | Toggle a container / show the full value of a leaf |
| `v` | Show the full value (long strings, embedded JSON pretty printed) |
| `e` | Expand everything under the cursor |
| `o` | Open all |
| `O` | Close all |
| `1`-`9` | Expand to depth N |
| `g`/`G` | Top/Bottom |
| `/` | Search keys and values in the whole document; `/$.path` jumps to a path |
| `n`/`N` | Next/previous match |
| `Esc` | Clear search highlight |
| `t` | Toggle type hints |
| `y` | Copy selected value (JSON) |
| `Y` | Copy selected path (usable with `jv -p`) |
| `?` | Help |
| `q` | Quit |

## Example

```bash
jv -i testdata/sample.json
```

## Notes
- `y`/`Y` copy to the system clipboard. When no clipboard tool is available
  (e.g. over SSH), `jv` falls back to the OSC 52 terminal sequence, which most
  modern terminals (and tmux with `set -g set-clipboard on`) support.
