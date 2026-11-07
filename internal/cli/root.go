package cli

import (
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"strings"

	"github.com/simota/jv/internal/diff"
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
	flat                bool
	compact             bool
	raw                 bool
	sortKeys            bool
	diff                bool
	path                string
	depth               int
	theme               string
	color               string
}

// exitError carries a process exit code. An empty message prints nothing.
type exitError struct {
	code int
	msg  string
}

func (e *exitError) Error() string { return e.msg }

// stdin is swappable for tests.
var (
	stdin           io.Reader = os.Stdin
	stdinIsTerminal           = func() bool { return term.IsTerminal(int(os.Stdin.Fd())) }
	runTUI                    = tui.Run
)

func Execute() {
	os.Exit(execute(os.Args[1:], os.Stdout, os.Stderr))
}

func execute(args []string, stdout, stderr io.Writer) int {
	cmd := newRootCmd()
	cmd.SetArgs(args)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	if err := cmd.Execute(); err != nil {
		var ee *exitError
		if errors.As(err, &ee) {
			if ee.msg != "" {
				_, _ = fmt.Fprintln(stderr, "Error: "+ee.msg)
			}
			return ee.code
		}
		_, _ = fmt.Fprintln(stderr, "Error: "+err.Error())
		return 1
	}
	return 0
}

func newRootCmd() *cobra.Command {
	opts := options{}
	cmd := &cobra.Command{
		Use:   "jv [OPTIONS] [FILE]\n  jv --diff OLD NEW",
		Short: "JSON viewer for the terminal",
		Long: `JSON viewer for the terminal.

Reads a JSON document (or JSON Lines / several concatenated documents) from
FILE or stdin ("-" also means stdin) and pretty prints it, or opens an
interactive collapsible viewer with -i.

Paths use the same syntax that the viewer shows in its footer:
  $.users[0].name   .users[-1]   users[*].email   $["key with space"]`,
		Example: `  jv data.json                      # pretty print
  jv -p '.items[*].id' data.json    # extract values by path
  jv -r -p .name data.json          # raw string output for scripts
  jv -f data.json | grep email      # greppable "path = value" lines
  jv -s data.json                   # inferred schema
  jv -i data.json                   # interactive viewer
  jv --diff old.json new.json       # structural diff (exit 1 if different)
  kubectl get pods -o json | jv -i -p '.items[*].status.phase'`,
		Args:          cobra.MaximumNArgs(2),
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			cmd.SilenceUsage = true
			if !cmd.Flags().Changed("color") && os.Getenv("NO_COLOR") != "" {
				opts.color = "never"
			}
			if opts.diff {
				if len(args) != 2 {
					return errors.New("--diff requires two inputs: jv --diff OLD NEW")
				}
				return runDiff(cmd, opts, args[0], args[1])
			}
			if len(args) == 2 {
				return errors.New("two inputs given; did you mean: jv --diff OLD NEW")
			}
			file := ""
			if len(args) == 1 {
				file = args[0]
			}
			return run(cmd, opts, file)
		},
	}

	f := cmd.Flags()
	f.BoolVarP(&opts.forceInteractive, "interactive", "i", false, "Force interactive mode")
	f.BoolVarP(&opts.forceNonInteractive, "no-interactive", "n", false, "Force non-interactive mode")
	f.BoolVarP(&opts.showType, "type", "t", false, "Show type hints")
	f.BoolVarP(&opts.schema, "schema", "s", false, "Show inferred schema (merges all array elements)")
	f.BoolVarP(&opts.flat, "flat", "f", false, "Print one 'path = value' line per leaf (greppable)")
	f.BoolVar(&opts.compact, "compact", false, "Print each document on a single line")
	f.BoolVarP(&opts.raw, "raw", "r", false, "Print string results without quotes")
	f.BoolVar(&opts.sortKeys, "sort-keys", false, "Sort object keys alphabetically (default: input order)")
	f.StringVarP(&opts.path, "path", "p", "", "Select values by path, e.g. '.items[*].name' (interactive: jump to them)")
	f.BoolVar(&opts.diff, "diff", false, "Compare two JSON inputs and list differences by path")
	f.IntVarP(&opts.depth, "depth", "d", 2, "Initial expand depth (interactive)")
	f.StringVar(&opts.theme, "theme", "dark", "Theme (dark/light)")
	f.StringVarP(&opts.color, "color", "c", "always", "Color (auto/always/never); NO_COLOR is honored when not set")

	return cmd
}

func validate(opts *options) error {
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
	return nil
}

func run(cmd *cobra.Command, opts options, file string) error {
	if err := validate(&opts); err != nil {
		return err
	}

	var query *parser.Query
	if opts.path != "" {
		q, err := parser.CompilePath(opts.path)
		if err != nil {
			return err
		}
		query = q
	}

	docs, err := loadDocuments(file, opts.sortKeys)
	if err != nil {
		return err
	}

	interactive := decideInteractive(opts)
	colorEnabled := decideColorEnabled(opts.color, interactive)

	if interactive {
		root := docs[0]
		if len(docs) > 1 {
			root = parser.WrapDocuments(docs)
		}
		var focus []*parser.Node
		if query != nil {
			for _, doc := range docs {
				focus = append(focus, query.Find(doc)...)
			}
			if len(focus) == 0 {
				return fmt.Errorf("path not found: %s", query)
			}
		}
		return runTUI(root, tui.Options{
			Depth:        opts.depth,
			Theme:        opts.theme,
			ColorEnabled: colorEnabled,
			ShowTypes:    opts.showType,
			Focus:        focus,
			FocusLabel:   opts.path,
		})
	}

	targets := docs
	if query != nil {
		targets = nil
		for _, doc := range docs {
			targets = append(targets, query.Find(doc)...)
		}
		if len(targets) == 0 {
			return fmt.Errorf("path not found: %s", query)
		}
	}

	if opts.schema {
		_, err = io.WriteString(cmd.OutOrStdout(), pipe.NewSchemaFormatter(colorEnabled).FormatMany(targets))
		return err
	}

	formatter := selectFormatter(opts, colorEnabled)
	out := pipe.NewWriter(cmd.OutOrStdout())
	for _, node := range targets {
		formatter.FormatTo(out, node)
	}
	return out.Flush()
}

func runDiff(cmd *cobra.Command, opts options, oldFile, newFile string) error {
	if err := validate(&opts); err != nil {
		return err
	}
	if oldFile == "-" && newFile == "-" {
		return errors.New("only one --diff input can be read from stdin")
	}
	load := func(file string) (*parser.Node, error) {
		docs, err := loadDocuments(file, false)
		if err != nil {
			return nil, err
		}
		root := docs[0]
		if len(docs) > 1 {
			root = parser.WrapDocuments(docs)
		}
		if opts.path == "" {
			return root, nil
		}
		q, err := parser.CompilePath(opts.path)
		if err != nil {
			return nil, err
		}
		found := q.Find(root)
		if len(found) == 0 {
			return nil, fmt.Errorf("path not found in %s: %s", displayName(file), opts.path)
		}
		if len(found) > 1 {
			return nil, fmt.Errorf("path %s matches %d values; --diff needs a single value", opts.path, len(found))
		}
		return found[0], nil
	}
	a, err := load(oldFile)
	if err != nil {
		return err
	}
	b, err := load(newFile)
	if err != nil {
		return err
	}
	changes := diff.Compare(a, b)
	colorEnabled := decideColorEnabled(opts.color, false)
	out := pipe.NewWriter(cmd.OutOrStdout())
	pipe.FormatDiffTo(out, changes, colorEnabled)
	if err := out.Flush(); err != nil {
		return err
	}
	if len(changes) > 0 {
		_, _ = fmt.Fprintln(cmd.ErrOrStderr(), "jv: "+diff.Summary(changes))
		return &exitError{code: 1}
	}
	return nil
}

func displayName(file string) string {
	if file == "" || file == "-" {
		return "stdin"
	}
	return file
}

func loadDocuments(file string, sortKeys bool) ([]*parser.Node, error) {
	data, err := readInput(file)
	if err != nil {
		return nil, err
	}
	docs, err := parser.ParseString(data)
	if err != nil {
		var se *parser.SyntaxError
		if errors.As(err, &se) {
			return nil, fmt.Errorf("%s: %w", displayName(file), err)
		}
		return nil, err
	}
	if sortKeys {
		for _, doc := range docs {
			parser.SortKeys(doc)
		}
	}
	return docs, nil
}

// readInput returns the whole input as a string. Reading straight into a
// string avoids the extra copy of the input that ParseBytes would make.
func readInput(file string) (string, error) {
	if file != "" && file != "-" {
		f, err := os.Open(file)
		if err != nil {
			return "", err
		}
		defer func() { _ = f.Close() }()
		var size int64
		if info, err := f.Stat(); err == nil && info.Mode().IsRegular() {
			size = info.Size()
		}
		return readAllString(f, size)
	}
	if file == "-" || !stdinIsTerminal() {
		return readAllString(stdin, 0)
	}
	return "", errors.New("no input provided. Try: jv path/to.json | cat file.json | jv | echo '{}' | jv")
}

// readAllString reads r until EOF. sizeHint is the expected size (0 when
// unknown).
func readAllString(r io.Reader, sizeHint int64) (string, error) {
	if sizeHint <= 0 {
		// stdin redirected from a file has a known size too
		if f, ok := r.(*os.File); ok {
			if info, err := f.Stat(); err == nil && info.Mode().IsRegular() {
				sizeHint = info.Size()
			}
		}
	}
	if sizeHint > 0 && sizeHint < math.MaxInt-512 {
		var b strings.Builder
		b.Grow(int(sizeHint) + 512) // room for a file that grows while it is read
		if _, err := io.Copy(&b, r); err != nil {
			return "", err
		}
		return b.String(), nil
	}
	// Unknown size (a pipe): collect chunks and copy them once, instead of
	// growing one buffer and copying it again at every step.
	var chunks [][]byte
	total := 0
	size := 64 << 10
	for {
		chunk := make([]byte, size)
		n, err := io.ReadFull(r, chunk)
		if n > 0 {
			chunks = append(chunks, chunk[:n])
			total += n
		}
		if err == io.EOF || err == io.ErrUnexpectedEOF {
			break
		}
		if err != nil {
			return "", err
		}
		if size < 16<<20 {
			size *= 2
		}
	}
	var b strings.Builder
	b.Grow(total)
	for _, c := range chunks {
		b.Write(c)
	}
	return b.String(), nil
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
	if opts.flat {
		return pipe.NewFlatFormatter(colorEnabled)
	}
	if opts.showType {
		return pipe.NewTypedFormatter(colorEnabled)
	}
	if opts.compact {
		return pipe.NewCompactFormatter(colorEnabled).WithRaw(opts.raw)
	}
	return pipe.NewPrettyFormatter(colorEnabled).WithRaw(opts.raw)
}
