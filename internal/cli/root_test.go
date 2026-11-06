package cli

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/simota/jv/internal/parser"
	"github.com/simota/jv/internal/tui"
)

func writeFile(t *testing.T, name, content string) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), name)
	if err := os.WriteFile(p, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	return p
}

func runCLI(t *testing.T, input string, args ...string) (string, string, int) {
	t.Helper()
	oldIn, oldTerm := stdin, stdinIsTerminal
	t.Cleanup(func() { stdin, stdinIsTerminal = oldIn, oldTerm })
	stdin = strings.NewReader(input)
	stdinIsTerminal = func() bool { return input == "" }
	var out, errOut bytes.Buffer
	code := execute(args, &out, &errOut)
	return out.String(), errOut.String(), code
}

const sample = "../../testdata/sample.json"

func TestPrettyDefaultKeepsInputOrder(t *testing.T) {
	out, _, code := runCLI(t, "", "-c", "never", sample)
	if code != 0 || !strings.HasPrefix(out, "{\n  \"name\": \"John Doe\",\n  \"age\": 30,") {
		t.Fatalf("code=%d out=%s", code, out)
	}
	sorted, _, _ := runCLI(t, "", "-c", "never", "--sort-keys", sample)
	if !strings.HasPrefix(sorted, "{\n  \"active\": true,") {
		t.Fatalf("sorted out=%s", sorted)
	}
}

func TestPathExtraction(t *testing.T) {
	cases := []struct {
		args []string
		want string
	}{
		{[]string{"-p", ".projects[*].name"}, "\"Alpha\"\n\"Beta\"\n"},
		{[]string{"-r", "-p", "$.address.city"}, "Tokyo\n"},
		{[]string{"--compact", "-p", ".address.lines"}, "[\"1-1-1 Chiyoda\",\"Building 2\"]\n"},
		{[]string{"-f", "-p", ".projects[-1]"}, "$.id = 2\n$.name = \"Beta\"\n$.status = null\n"},
		{[]string{"-s", "-p", ".projects"}, "[\n  {\n    \"id\": number,\n    \"name\": string,\n    \"status\": string | null\n  }\n]\n"},
	}
	for _, c := range cases {
		args := append([]string{"-c", "never"}, c.args...)
		args = append(args, sample)
		out, errOut, code := runCLI(t, "", args...)
		if code != 0 || out != c.want {
			t.Errorf("%v: code=%d out=%q err=%q", c.args, code, out, errOut)
		}
	}
}

func TestPathErrors(t *testing.T) {
	_, errOut, code := runCLI(t, "", "-p", ".missing", sample)
	if code != 1 || !strings.Contains(errOut, "path not found: .missing") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
	_, errOut, code = runCLI(t, "", "-p", ".a[", sample)
	if code != 1 || !strings.Contains(errOut, "invalid path") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
	if strings.Contains(errOut, "Usage:") {
		t.Fatal("runtime errors must not print usage")
	}
}

func TestStdinJSONLines(t *testing.T) {
	out, _, code := runCLI(t, "{\"a\":1}\n{\"a\":2,\"b\":\"x\"}\n", "-c", "never", "--compact")
	if code != 0 || out != "{\"a\":1}\n{\"a\":2,\"b\":\"x\"}\n" {
		t.Fatalf("code=%d out=%q", code, out)
	}
	out, _, _ = runCLI(t, "{\"a\":1}\n{\"a\":2,\"b\":\"x\"}\n", "-c", "never", "-r", "-p", ".a")
	if out != "1\n2\n" {
		t.Fatalf("out=%q", out)
	}
	out, _, _ = runCLI(t, "{\"a\":1}\n{\"a\":2,\"b\":\"x\"}\n", "-c", "never", "-s")
	if out != "{\n  \"a\": number,\n  \"b\"?: string\n}\n" {
		t.Fatalf("schema out=%q", out)
	}
	out, _, _ = runCLI(t, "[1]", "-c", "never", "--compact", "-")
	if out != "[1]\n" {
		t.Fatalf("dash stdin out=%q", out)
	}
}

func TestSyntaxErrorReportsPosition(t *testing.T) {
	p := writeFile(t, "bad.json", "{\n  \"a\": 1,\n  \"b\": \n}")
	_, errOut, code := runCLI(t, "", p)
	if code != 1 || !strings.Contains(errOut, "bad.json: invalid JSON at line 4, column 1") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
}

func TestNoColorEnv(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	out, _, _ := runCLI(t, "", "--compact", "-p", ".age", sample)
	if out != "30\n" {
		t.Fatalf("NO_COLOR ignored: %q", out)
	}
	out, _, _ = runCLI(t, "", "--compact", "-c", "always", "-p", ".age", sample)
	if !strings.Contains(out, "\x1b[") {
		t.Fatalf("explicit --color always should win: %q", out)
	}
}

func TestDiff(t *testing.T) {
	a := writeFile(t, "a.json", `{"name":"x","n":1,"list":[1,2],"cfg":{"on":true}}`)
	b := writeFile(t, "b.json", `{"cfg":{"on":false},"name":"x","n":1.0,"list":[1],"new":"v"}`)
	out, errOut, code := runCLI(t, "", "-c", "never", "--diff", a, b)
	want := "- $.list[1]: 2\n~ $.cfg.on: true -> false\n+ $.new: \"v\"\n"
	if code != 1 || out != want || !strings.Contains(errOut, "1 changed, 1 added, 1 removed") {
		t.Fatalf("code=%d out=%q err=%q", code, out, errOut)
	}
	out, _, code = runCLI(t, "", "--diff", a, a)
	if code != 0 || out != "" {
		t.Fatalf("identical: code=%d out=%q", code, out)
	}
	out, _, code = runCLI(t, `{"cfg":{"on":true}}`, "-c", "never", "--diff", "-p", ".cfg", a, "-")
	if code != 0 || out != "" {
		t.Fatalf("diff with path and stdin: code=%d out=%q", code, out)
	}
	_, errOut, code = runCLI(t, "", "--diff", a)
	if code != 1 || !strings.Contains(errOut, "requires two inputs") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
	_, errOut, code = runCLI(t, "", a, b)
	if code != 1 || !strings.Contains(errOut, "--diff") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
}

func TestInteractivePassesFocus(t *testing.T) {
	old := runTUI
	t.Cleanup(func() { runTUI = old })
	var gotRoot *parser.Node
	var gotOpts tui.Options
	runTUI = func(root *parser.Node, opts tui.Options) error {
		gotRoot, gotOpts = root, opts
		return nil
	}
	_, errOut, code := runCLI(t, "{\"a\":1}\n{\"a\":2}\n", "-i", "-p", ".a")
	if code != 0 {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
	if gotRoot.Type != parser.TypeArray || len(gotRoot.Children) != 2 {
		t.Fatalf("multi-doc root not wrapped: %+v", gotRoot)
	}
	if len(gotOpts.Focus) != 2 || gotOpts.Focus[1].Path() != "$[1].a" {
		t.Fatalf("focus = %+v", gotOpts.Focus)
	}
	_, errOut, code = runCLI(t, "{}", "-i", "-p", ".zzz")
	if code != 1 || !strings.Contains(errOut, "path not found") {
		t.Fatalf("code=%d err=%q", code, errOut)
	}
}

func TestFlagValidation(t *testing.T) {
	for _, args := range [][]string{{"-i", "-n"}, {"--theme", "blue"}, {"-c", "rainbow"}} {
		_, errOut, code := runCLI(t, "{}", args...)
		if code != 1 || errOut == "" {
			t.Errorf("%v: code=%d err=%q", args, code, errOut)
		}
	}
}
