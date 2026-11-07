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

func TestNumbersEqual(t *testing.T) {
	equal := [][2]string{
		{"1", "1.0"}, {"1", "1e0"}, {"10e-1", "1"}, {"0", "-0"}, {"0.0e5", "0"},
		{"120", "1.2e2"}, {"-0.5", "-5E-1"}, {"100", "1e+2"}, {"0.001", "1e-3"},
		{"123456789012345678901234567890", "1.2345678901234567890123456789e29"},
		// leading zeros in the exponent do not count toward its length limit
		{"1e0000000000000000000000000000001", "10"},
		{"1e-0000000000000000000000000000000001", "0.1"},
		{"5E+0000000000000000000000000000000", "5"},
	}
	for _, c := range equal {
		if !numbersEqual(c[0], c[1]) {
			t.Errorf("%s == %s: got false", c[0], c[1])
		}
	}
	// These differ beyond 64-bit binary precision, which used to hide them.
	different := [][2]string{
		{"1", "2"}, {"1", "-1"}, {"0.1", "0.01"}, {"1e400", "1e401"},
		{"123456789012345678901", "123456789012345678902"},
		{"1.00000000000000000001", "1"},
		{"0.1000000000000000000001", "0.1"},
		{"-9223372036854775809", "-9223372036854775808"},
	}
	for _, c := range different {
		if numbersEqual(c[0], c[1]) {
			t.Errorf("%s != %s: got true", c[0], c[1])
		}
	}
}

func TestCompareLargeNumbers(t *testing.T) {
	got := describe(Compare(parse(t, `{"id":123456789012345678901}`), parse(t, `{"id":123456789012345678902}`)))
	if got != "~$.id" {
		t.Fatalf("got %q want %q", got, "~$.id")
	}
}
