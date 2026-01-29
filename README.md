# jv

A simple JSON viewer written in Go. Supports pretty formatting, optional type hints, and a collapsible interactive TUI.

## Features
- Beautiful JSON pretty printing
- Type hints (use `-t` in pipe mode)
- Collapsible interactive TUI
- Search and copy for paths/values

## Install / Build

```bash
make build
```

This produces `./jv`.

## Usage

### Pipe mode (default)

```bash
jv file.json
cat file.json | jv
```

With type hints:

```bash
cat file.json | jv -t
```

Schema view:

```bash
cat file.json | jv -s
```

### Interactive mode (TUI)

```bash
jv -i file.json
cat file.json | jv -i
```

You can enable type hints initially with `-t`, and toggle them in TUI with `t`.

## Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--interactive` | `-i` | Force interactive mode | false |
| `--no-interactive` | `-n` | Force pipe mode | false |
| `--type` | `-t` | Show type hints | false |
| `--schema` | `-s` | Schema mode | false |
| `--depth` | `-d` | Initial expand depth (TUI) | 2 |
| `--theme` |  | Theme (dark/light) | dark |
| `--color` | `-c` | Color (auto/always/never) | always |

## TUI key bindings

| Key | Action |
|-----|--------|
| `↑`/`k` | Move up |
| `↓`/`j` | Move down |
| `PgUp`/`PgDn` | Page up/down |
| `Ctrl+u`/`Ctrl+d` | Page up/down |
| `←`/`h` | Collapse |
| `→`/`l` | Expand |
| `Enter`/`Space` | Toggle |
| `o` | Open all |
| `O` | Close all |
| `1`-`9` | Expand to depth N |
| `g`/`G` | Top/Bottom |
| `/` | Search |
| `t` | Toggle type hints |
| `y` | Copy selected value |
| `?` | Help |
| `q` | Quit |

## Example

```bash
jv -i testdata/sample.json
```

## Notes
- `y` copies to the system clipboard. This may not work in some environments.
