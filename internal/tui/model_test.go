package tui

import (
	"errors"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/simota/jv/internal/parser"
)

const doc = `{
  "name": "jv",
  "deep": {"level1": {"level2": {"needle": "found me"}}},
  "items": [{"id": 1, "tag": "x"}, {"id": 2, "tag": "needle-ish"}],
  "payload": "{\"inner\": [1, 2]}",
  "long": "` + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaEND" + `"
}`

func newTestModel(t *testing.T, opts Options) (Model, *[]string) {
	t.Helper()
	root, err := parser.Parse(strings.NewReader(doc))
	if err != nil {
		t.Fatal(err)
	}
	m := NewModel(root, opts)
	copied := &[]string{}
	m.copyFn = func(s string) error { *copied = append(*copied, s); return nil }
	next, _ := m.Update(tea.WindowSizeMsg{Width: 60, Height: 20})
	return next.(Model), copied
}

func press(m Model, keys ...string) Model {
	for _, k := range keys {
		var msg tea.KeyMsg
		switch k {
		case "enter":
			msg = tea.KeyMsg{Type: tea.KeyEnter}
		case "esc":
			msg = tea.KeyMsg{Type: tea.KeyEsc}
		case " ":
			msg = tea.KeyMsg{Type: tea.KeySpace, Runes: []rune{' '}}
		default:
			msg = tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(k)}
		}
		next, _ := m.Update(msg)
		m = next.(Model)
	}
	return m
}

func typeSearch(m Model, term string) Model {
	m = press(m, "/")
	for _, r := range term {
		next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{r}})
		m = next.(Model)
	}
	return press(m, "enter")
}

func TestSearchFindsCollapsedNodes(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = typeSearch(m, "found me")
	if got := m.currentNode().Path(); got != "$.deep.level1.level2.needle" {
		t.Fatalf("cursor at %s (status %q)", got, m.statusMsg)
	}
	if !strings.Contains(m.statusMsg, "Match 1/1") {
		t.Fatalf("status = %q", m.statusMsg)
	}
}

func TestSearchNextPrevWraps(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = typeSearch(m, "needle")
	// key "needle" and value "needle-ish" both match, in document order
	if m.currentNode().Path() != "$.deep.level1.level2.needle" || !strings.Contains(m.statusMsg, "1/2") {
		t.Fatalf("first: %s %q", m.currentNode().Path(), m.statusMsg)
	}
	m = press(m, "n")
	if m.currentNode().Path() != "$.items[1].tag" || !strings.Contains(m.statusMsg, "2/2") {
		t.Fatalf("next: %s %q", m.currentNode().Path(), m.statusMsg)
	}
	m = press(m, "n")
	if m.currentNode().Path() != "$.deep.level1.level2.needle" {
		t.Fatalf("wrap: %s", m.currentNode().Path())
	}
	m = press(m, "N")
	if m.currentNode().Path() != "$.items[1].tag" {
		t.Fatalf("prev wrap: %s", m.currentNode().Path())
	}
	m = press(m, "esc")
	if len(m.matches) != 0 || m.statusMsg != "Search cleared" {
		t.Fatalf("esc did not clear: %d %q", len(m.matches), m.statusMsg)
	}
}

func TestSearchByPathAndNotFound(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 0})
	m = typeSearch(m, "$.items[*].id")
	if m.currentNode().Path() != "$.items[0].id" || len(m.matches) != 2 {
		t.Fatalf("path search: %s %d", m.currentNode().Path(), len(m.matches))
	}
	m = typeSearch(m, "zzz-none")
	if !strings.Contains(m.statusMsg, "not found") || len(m.matches) != 0 {
		t.Fatalf("status %q", m.statusMsg)
	}
	m = typeSearch(m, "$.a[")
	if !strings.Contains(m.statusMsg, "invalid path") {
		t.Fatalf("status %q", m.statusMsg)
	}
}

func TestSearchIgnoresArrayIndexKeys(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = typeSearch(m, "1")
	for _, n := range m.matches {
		if n.Parent != nil && n.Parent.Type == parser.TypeArray && n.Type == parser.TypeObject {
			t.Fatalf("array element matched by its index: %s", n.Path())
		}
	}
	if len(m.matches) != 3 { // "level1" key, id 1, and the payload string
		t.Fatalf("matches = %d", len(m.matches))
	}
}

func TestFocusOption(t *testing.T) {
	root, _ := parser.Parse(strings.NewReader(doc))
	q, _ := parser.CompilePath(".items[*].tag")
	m := NewModel(root, Options{Depth: 0, Focus: q.Find(root), FocusLabel: ".items[*].tag"})
	if m.currentNode().Path() != "$.items[0].tag" || !strings.Contains(m.statusMsg, "1/2") {
		t.Fatalf("focus: %s %q", m.currentNode().Path(), m.statusMsg)
	}
	m = press(m, "n")
	if m.currentNode().Path() != "$.items[1].tag" {
		t.Fatalf("n after focus: %s", m.currentNode().Path())
	}
}

func TestCopyValueAndPath(t *testing.T) {
	m, copied := newTestModel(t, Options{Depth: 2})
	m = typeSearch(m, "$.items[0]")
	m = press(m, "y", "Y")
	if len(*copied) != 2 {
		t.Fatalf("copied = %v", *copied)
	}
	if (*copied)[0] != "{\n  \"id\": 1,\n  \"tag\": \"x\"\n}" {
		t.Fatalf("value copy (order must be preserved): %q", (*copied)[0])
	}
	if (*copied)[1] != "$.items[0]" || m.statusMsg != "Copied path $.items[0]" {
		t.Fatalf("path copy: %q %q", (*copied)[1], m.statusMsg)
	}
	m.copyFn = func(string) error { return errors.New("no clipboard") }
	m = press(m, "y")
	if m.statusMsg != "Copy failed: no clipboard" {
		t.Fatalf("status = %q", m.statusMsg)
	}
}

func TestDetailViewShowsFullAndEmbeddedJSON(t *testing.T) {
	m, copied := newTestModel(t, Options{Depth: 2})
	m = typeSearch(m, "$.long")
	if strings.Contains(m.View(), "END") {
		t.Fatal("precondition: long value should be cut off in tree view")
	}
	m = press(m, "enter")
	if !m.detailMode || !strings.Contains(m.View(), "END") {
		t.Fatalf("detail view should show full value:\n%s", m.View())
	}
	m = press(m, "esc")
	if m.detailMode {
		t.Fatal("esc should close detail view")
	}
	m = typeSearch(m, "$.payload")
	m = press(m, "v")
	view := m.View()
	if !strings.Contains(view, "Embedded JSON:") || !strings.Contains(view, "\"inner\": [") {
		t.Fatalf("embedded JSON not shown:\n%s", view)
	}
	m = press(m, "y")
	if (*copied)[len(*copied)-1] != `{"inner": [1, 2]}` {
		t.Fatalf("detail copy should copy raw string: %q", (*copied)[len(*copied)-1])
	}
}

func TestCollapsedContainerIsOneLineWithSummary(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	view := m.View()
	if !strings.Contains(view, `"deep": {… 1 key},`) || !strings.Contains(view, `"items": [… 2 items],`) {
		t.Fatalf("view:\n%s", view)
	}
}

func TestExpandSubtreeAndDepthKeepsCursor(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = typeSearch(m, "$.deep")
	m = press(m, "esc", "e")
	if !strings.Contains(m.View(), `"needle": "found me"`) {
		t.Fatalf("e should expand subtree:\n%s", m.View())
	}
	m = typeSearch(m, "found me")
	m = press(m, "1")
	if m.currentNode().Path() != "$.deep" {
		t.Fatalf("cursor should move to nearest visible ancestor, got %s", m.currentNode().Path())
	}
}

func TestNavigationBasics(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = press(m, "j", "j")
	if m.currentNode().Path() != "$.deep" {
		t.Fatalf("j j -> %s", m.currentNode().Path())
	}
	m = press(m, "l", "j")
	if m.currentNode().Path() != "$.deep.level1" {
		t.Fatalf("l j -> %s", m.currentNode().Path())
	}
	m = press(m, "h")
	if m.currentNode().Path() != "$.deep" {
		t.Fatalf("h on leaf-ish collapsed node -> %s", m.currentNode().Path())
	}
	m = press(m, "G")
	if m.currentNode().Path() != "$.long" {
		t.Fatalf("G -> %s", m.currentNode().Path())
	}
	m = press(m, "g", "?")
	if !strings.Contains(m.View(), "copy path") {
		t.Fatal("help should be shown")
	}
	m = press(m, "x")
	if m.helpMode {
		t.Fatal("any key should close help")
	}
	m = press(m, "n")
	if m.statusMsg != "No active search (press /)" {
		t.Fatalf("status %q", m.statusMsg)
	}
}

// Strings from untrusted input must not reach the terminal as control
// characters: ESC, BEL, CR or C1 codes would let a document set the window
// title, write the clipboard (OSC 52) or overwrite what is on screen.
func TestDetailViewEscapesControlCharacters(t *testing.T) {
	src := `{"evil": "a\u001b]0;pwned\u0007b\rc\u009bd\u007fe\tf\ng"}`
	root, err := parser.Parse(strings.NewReader(src))
	if err != nil {
		t.Fatal(err)
	}
	m := NewModel(root, Options{Depth: 1})
	next, _ := m.Update(tea.WindowSizeMsg{Width: 80, Height: 20})
	m = press(next.(Model), "j", "v")
	if !m.detailMode {
		t.Fatal("detail view did not open")
	}
	view := m.View()
	for _, bad := range []string{"\x1b", "\x07", "\r", "\u009b", "\x7f"} {
		if strings.Contains(view, bad) {
			t.Errorf("detail view contains raw %q:\n%q", bad, view)
		}
	}
	for _, want := range []string{`a\u001b]0;pwned\u0007b\rc\u009bd\u007fe`, "\ng"} {
		if !strings.Contains(view, want) {
			t.Errorf("detail view lacks %q:\n%q", want, view)
		}
	}
	// copying still yields the exact value
	if got := m.detailCopyText(); got != "a\x1b]0;pwned\x07b\rc\u009bd\x7fe\tf\ng" {
		t.Errorf("copy text = %q", got)
	}
}

// The tree, header and footer show quoted keys, values and paths. JSON
// quoting escapes C0 controls but not DEL or C1 (U+0080..U+009F), which some
// terminals treat as control sequences (U+009B is CSI).
func TestTreeAndFooterEscapeC1Controls(t *testing.T) {
	src := `{"k\u009b2J": "v\u009b31m\u007f"}`
	root, err := parser.Parse(strings.NewReader(src))
	if err != nil {
		t.Fatal(err)
	}
	m := NewModel(root, Options{Depth: 1})
	var copied string
	m.copyFn = func(s string) error { copied = s; return nil }
	next, _ := m.Update(tea.WindowSizeMsg{Width: 80, Height: 10})
	m = press(next.(Model), "j")
	view := m.View()
	for _, bad := range []string{"\u009b", "\x7f"} {
		if strings.Contains(view, bad) {
			t.Errorf("view contains raw %q:\n%q", bad, view)
		}
	}
	for _, want := range []string{`"k\u009b2J": "v\u009b31m\u007f"`, `Path: $["k\u009b2J"]`} {
		if !strings.Contains(view, want) {
			t.Errorf("view lacks %q:\n%q", want, view)
		}
	}
	// the copied path is the real one, usable with jv -p
	m = press(m, "Y")
	if copied != "$[\"k\u009b2J\"]" {
		t.Fatalf("copied %q", copied)
	}
}

func TestDisplaySafe(t *testing.T) {
	cases := map[string]string{
		"plain":              "plain",
		"日本語 ok":             "日本語 ok",
		"tab\tnl\n":          "tab\tnl\n",
		"\x1b[31m":           `\u001b[31m`,
		"a\rb":               `a\rb`,
		"\x00\x07\x7f":       `\u0000\u0007\u007f`,
		"\u0085\u009f\u00a0": `\u0085\u009f` + "\u00a0",
	}
	for in, want := range cases {
		if got := displaySafe(in); got != want {
			t.Errorf("displaySafe(%q) = %q, want %q", in, got, want)
		}
	}
}

// The detail-mode header shows the path, and a search highlight re-renders
// the value; both must be escaped too.
func TestHeaderAndHighlightEscapeControls(t *testing.T) {
	src := `{"k\u009b]0;x\u007f": "hit\u009b2J"}`
	root, err := parser.Parse(strings.NewReader(src))
	if err != nil {
		t.Fatal(err)
	}
	m := NewModel(root, Options{Depth: 1})
	m.copyFn = func(string) error { return nil }
	next, _ := m.Update(tea.WindowSizeMsg{Width: 100, Height: 10})
	m = typeSearch(next.(Model), "hit")
	view := m.View()
	if strings.Contains(view, "\u009b") || !strings.Contains(view, `"hit\u009b2J"`) {
		t.Errorf("highlighted value not escaped:\n%q", view)
	}
	m = press(m, "v")
	view = m.View()
	for _, bad := range []string{"\x7f", "\u009b"} {
		if strings.Contains(view, bad) {
			t.Errorf("detail view contains raw %q:\n%q", bad, view)
		}
	}
	if !strings.Contains(view, `jv - Value: $["k\u009b]0;x\u007f"]`) {
		t.Errorf("header path not escaped:\n%q", view)
	}
}
