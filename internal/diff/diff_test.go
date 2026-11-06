package diff

import (
	"strings"
	"testing"

	"github.com/simota/jv/internal/parser"
)

func parse(t *testing.T, s string) *parser.Node {
	t.Helper()
	n, err := parser.Parse(strings.NewReader(s))
	if err != nil {
		t.Fatal(err)
	}
	return n
}

func describe(changes []Change) string {
	out := []string{}
	for _, c := range changes {
		out = append(out, c.Kind.Symbol()+c.Path)
	}
	return strings.Join(out, " ")
}

func TestCompare(t *testing.T) {
	cases := []struct{ a, b, want string }{
		{`{"a":1}`, `{"a":1}`, ``},
		{`{"a":1,"b":2}`, `{"b":2,"a":1}`, ``}, // key order ignored
		{`{"n":1.0}`, `{"n":1e0}`, ``},         // numerically equal
		{`{"n":1}`, `{"n":"1"}`, `~$.n`},       // type change
		{`{"a":{"x":1,"y":2}}`, `{"a":{"x":3,"z":4}}`, `~$.a.x -$.a.y +$.a.z`},
		{`[1,2,3]`, `[1,5]`, `~$[1] -$[2]`},
		{`[1]`, `[1,{"k":true}]`, `+$[1]`},
		{`{"odd key":null}`, `{}`, `-$["odd key"]`},
		{`"x"`, `"y"`, `~$`},
		{`{"a":[]}`, `{"a":{}}`, `~$.a`},
	}
	for _, c := range cases {
		got := describe(Compare(parse(t, c.a), parse(t, c.b)))
		if got != c.want {
			t.Errorf("%s vs %s: got %q want %q", c.a, c.b, got, c.want)
		}
	}
}

func TestSummary(t *testing.T) {
	ch := Compare(parse(t, `{"a":1,"b":1}`), parse(t, `{"a":2,"c":1,"d":1}`))
	if got := Summary(ch); got != "1 changed, 2 added, 1 removed" {
		t.Fatalf("summary = %q", got)
	}
	if got := Summary(nil); got != "no differences" {
		t.Fatalf("summary = %q", got)
	}
}
