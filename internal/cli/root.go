package cli

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"

	"github.com/simota/jv/internal/parser"
	"github.com/simota/jv/internal/pipe"
	"github.com/simota/jv/internal/tui"
	"github.com/spf13/cobra"
	"golang.org/x/term"
)

type options struct {
	forceInteractive    bool
	forceNonInteractive bool
	showType            bool
	schema              bool
	depth               int
	theme               string
	color               string
}

func Execute() {
	if err := newRootCmd().Execute(); err != nil {
		os.Exit(1)
	}
}

func newRootCmd() *cobra.Command {
	opts := options{}
	cmd := &cobra.Command{
		Use:   "jv [OPTIONS] [FILE]",
		Short: "JSON viewer for the terminal",
		Args:  cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			file := ""
			if len(args) == 1 {
				file = args[0]
			}
			return run(cmd, opts, file)
		},
	}

	cmd.Flags().BoolVarP(&opts.forceInteractive, "interactive", "i", false, "Force interactive mode")
	cmd.Flags().BoolVarP(&opts.forceNonInteractive, "no-interactive", "n", false, "Force non-interactive mode")
	cmd.Flags().BoolVarP(&opts.showType, "type", "t", false, "Show type hints")
	cmd.Flags().BoolVarP(&opts.schema, "schema", "s", false, "Show schema mode")
	cmd.Flags().IntVarP(&opts.depth, "depth", "d", 2, "Initial expand depth (interactive)")
	cmd.Flags().StringVar(&opts.theme, "theme", "dark", "Theme (dark/light)")
	cmd.Flags().StringVarP(&opts.color, "color", "c", "always", "Color (auto/always/never)")

	return cmd
}

func run(cmd *cobra.Command, opts options, file string) error {
	if opts.forceInteractive && opts.forceNonInteractive {
		return errors.New("cannot use --interactive and --no-interactive together")
	}
	if opts.theme != "dark" && opts.theme != "light" {
		return fmt.Errorf("invalid theme: %s", opts.theme)
	}
	if opts.color != "auto" && opts.color != "always" && opts.color != "never" {
		return fmt.Errorf("invalid color mode: %s", opts.color)
	}
	if opts.depth < 0 {
		opts.depth = 0
	}

	data, err := readInput(file)
	if err != nil {
		return err
	}

	root, err := parser.Parse(bytes.NewReader(data))
	if err != nil {
		return err
	}

	interactive := decideInteractive(opts)
	colorEnabled := decideColorEnabled(opts.color, interactive)

	if interactive {
		return tui.Run(root, tui.Options{Depth: opts.depth, Theme: opts.theme, ColorEnabled: colorEnabled, ShowTypes: opts.showType})
	}

	formatter := selectFormatter(opts, colorEnabled)
	output := formatter.Format(root)
	_, err = io.WriteString(cmd.OutOrStdout(), output)
	return err
}

func readInput(file string) ([]byte, error) {
	if file != "" {
		return os.ReadFile(file)
	}
	if !term.IsTerminal(int(os.Stdin.Fd())) {
		return io.ReadAll(os.Stdin)
	}
	return nil, errors.New("no input provided. Try: jv path/to.json | cat file.json | jv | echo '{}' | jv")
}

func decideInteractive(opts options) bool {
	if opts.forceInteractive {
		return true
	}
	if opts.forceNonInteractive {
		return false
	}
	return false
}

func decideColorEnabled(mode string, interactive bool) bool {
	switch mode {
	case "always":
		return true
	case "never":
		return false
	default:
		if interactive {
			return true
		}
		return term.IsTerminal(int(os.Stdout.Fd()))
	}
}

func selectFormatter(opts options, colorEnabled bool) pipe.Formatter {
	if opts.schema {
		return pipe.NewSchemaFormatter(colorEnabled)
	}
	if opts.showType {
		return pipe.NewTypedFormatter(colorEnabled)
	}
	return pipe.NewPrettyFormatter(colorEnabled)
}
