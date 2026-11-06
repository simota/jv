package parser

import (
	"strings"
	"testing"
)

const pathDoc = `{
  "name": "jv",
  "items": [{"id": 1, "tags": ["a"]}, {"id": 2}, {"id": 3}],
  "misc": {"key with space": 1, "kebab-key": 2, "a.b": 3}
}`

func findValues(t *testing.T, root *Node, expr string) []string {
	t.Helper()
	q, err := CompilePath(expr)
	if err != nil {
		t.Fatalf("compile %q: %v", expr, err)
	}
	out := []string{}
	for _, n := range q.Find(root) {
		out = append(out, Marshal(n, ""))
	}
	return out
}

func TestQueryFind(t *testing.T) {
	root := mustParse(t, pathDoc)
	cases := map[string]string{
		"$":                        Marshal(root, ""),
		".":                        Marshal(root, ""),
		"":                         Marshal(root, ""),
		"$.name":                   `"jv"`,
		".name":                    `"jv"`,
		"name":                     `"jv"`,
		"items[1].id":              `2`,
		"$.items[-1].id":           `3`,
		"$.items[*].id":            `1|2|3`,
		".items.*.id":              `1|2|3`,
		`$.misc["key with space"]`: `1`,
		`$.misc.kebab-key`:         `2`,
		`$.misc["a.b"]`:            `3`,
		`$.misc."a.b"`:             `3`,
		"$.items[0].tags[0]":       `"a"`,
		"$.missing":                ``,
		"$.items[10]":              ``,
		"$.name[0]":                ``,
		"$.items.id":               ``,
		"$.misc[*]":                `1|2|3`,
		".items.[1].id":            `2`,
	}
	for expr, want := range cases {
		got := strings.Join(findValues(t, root, expr), "|")
		if got != want {
			t.Errorf("%q => %q, want %q", expr, got, want)
		}
	}
}

func TestQueryRoundTripsNodePath(t *testing.T) {
	root := mustParse(t, pathDoc)
	Walk(root, func(n *Node) bool {
		q, err := CompilePath(n.Path())
		if err != nil {
			t.Fatalf("compile %s: %v", n.Path(), err)
		}
		got := q.Find(root)
		if len(got) != 1 || got[0] != n {
			t.Errorf("path %s did not resolve to its node", n.Path())
		}
		return true
	})
}

func TestCompilePathErrors(t *testing.T) {
	for _, expr := range []string{"$.a[", "$.a[x]", `$.a["b`, "$..a", "$.a]"} {
		if _, err := CompilePath(expr); err == nil {
			t.Errorf("expected error for %q", expr)
		}
	}
}
