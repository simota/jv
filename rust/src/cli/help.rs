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

// Texts of the hidden `completion` command family that cobra adds (captured
// from the Go build).

pub const COMPLETION_LONG: &str = r##"Generate the autocompletion script for jv for the specified shell.
See each sub-command's help for details on how to use the generated script."##;

pub const COMPLETION_USAGE: &str = r##"Usage:
  jv completion [command]

Available Commands:
  bash        Generate the autocompletion script for bash
  fish        Generate the autocompletion script for fish
  powershell  Generate the autocompletion script for powershell
  zsh         Generate the autocompletion script for zsh

Flags:
  -h, --help   help for completion

Use "jv completion [command] --help" for more information about a command.
"##;

pub const BASH_LONG: &str = r##"Generate the autocompletion script for the bash shell.

This script depends on the 'bash-completion' package.
If it is not installed already, you can install it via your OS's package manager.

To load completions in your current shell session:

	source <(jv completion bash)

To load completions for every new session, execute once:

#### Linux:

	jv completion bash > /etc/bash_completion.d/jv

#### macOS:

	jv completion bash > $(brew --prefix)/etc/bash_completion.d/jv

You will need to start a new shell for this setup to take effect."##;

pub const BASH_USAGE: &str = r##"Usage:
  jv completion bash

Flags:
  -h, --help              help for bash
      --no-descriptions   disable completion descriptions
"##;

pub const ZSH_LONG: &str = r##"Generate the autocompletion script for the zsh shell.

If shell completion is not already enabled in your environment you will need
to enable it.  You can execute the following once:

	echo "autoload -U compinit; compinit" >> ~/.zshrc

To load completions in your current shell session:

	source <(jv completion zsh)

To load completions for every new session, execute once:

#### Linux:

	jv completion zsh > "${fpath[1]}/_jv"

#### macOS:

	jv completion zsh > $(brew --prefix)/share/zsh/site-functions/_jv

You will need to start a new shell for this setup to take effect."##;

pub const ZSH_USAGE: &str = r##"Usage:
  jv completion zsh [flags]

Flags:
  -h, --help              help for zsh
      --no-descriptions   disable completion descriptions
"##;

pub const FISH_LONG: &str = r##"Generate the autocompletion script for the fish shell.

To load completions in your current shell session:

	jv completion fish | source

To load completions for every new session, execute once:

	jv completion fish > ~/.config/fish/completions/jv.fish

You will need to start a new shell for this setup to take effect."##;

pub const FISH_USAGE: &str = r##"Usage:
  jv completion fish [flags]

Flags:
  -h, --help              help for fish
      --no-descriptions   disable completion descriptions
"##;

pub const POWERSHELL_LONG: &str = r##"Generate the autocompletion script for powershell.

To load completions in your current shell session:

	jv completion powershell | Out-String | Invoke-Expression

To load completions for every new session, add the output of the above command
to your powershell profile."##;

pub const POWERSHELL_USAGE: &str = r##"Usage:
  jv completion powershell [flags]

Flags:
  -h, --help              help for powershell
      --no-descriptions   disable completion descriptions
"##;

/// Usage of the hidden `__complete` command.
pub const COMPLETE_USAGE: &str = r##"Usage:
  jv __complete [command-line]

Aliases:
  __complete, __completeNoDesc

Flags:
  -h, --help   help for __complete
"##;
