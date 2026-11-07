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

// Objects above smallObject are matched with maps, smaller ones by linear
// search; both must give the same result, including for duplicate keys
// (which only hand-built trees can contain: the last one wins).
func TestCompareObjectSizesAndDuplicateKeys(t *testing.T) {
	build := func(n int, changed int) (string, string) {
		var a, b strings.Builder
		a.WriteString("{")
		b.WriteString("{")
		for i := 0; i < n; i++ {
			if i > 0 {
				a.WriteString(",")
				b.WriteString(",")
			}
			k := `"k` + strings.Repeat("x", i) + `":`
			a.WriteString(k + "1")
			if i == changed {
				b.WriteString(k + "2")
			} else {
				b.WriteString(k + "1")
			}
		}
		a.WriteString(`,"gone":1}`)
		b.WriteString(`,"new":1}`)
		return a.String(), b.String()
	}
	for _, n := range []int{1, 5, 7, 8, 9, 20} {
		a, b := build(n, n-1)
		want := "~$.k" + strings.Repeat("x", n-1) + " -$.gone +$.new"
		if got := describe(Compare(parse(t, a), parse(t, b))); got != want {
			t.Errorf("n=%d: got %q want %q", n, got, want)
		}
	}

	dup := func(n int) *parser.Node {
		obj := &parser.Node{Type: parser.TypeObject}
		for i := 0; i < n; i++ {
			obj.Children = append(obj.Children, &parser.Node{Key: "k" + strings.Repeat("x", i), Type: parser.TypeNumber, Value: "1", Parent: obj})
		}
		obj.Children = append(obj.Children, &parser.Node{Key: "k", Type: parser.TypeNumber, Value: "2", Parent: obj})
		return obj
	}
	for _, n := range []int{2, 20} {
		a, b := dup(n), dup(n)
		b.Children[0].Value = "2"
		// Both "k" children of the old side are compared with the last "k"
		// of the new side (value 2).
		if got := describe(Compare(a, b)); got != "~$.k" {
			t.Errorf("n=%d duplicate keys: got %q want ~$.k", n, got)
		}
		if got := describe(Compare(b, a)); got != "" {
			t.Errorf("n=%d duplicate keys reversed: got %q want no changes", n, got)
		}
	}
}

// Strings compare by their JSON literal: invalid UTF-8 (only possible in
// hand-built trees) prints as �, so it equals a real U+FFFD.
func TestCompareStringsByLiteral(t *testing.T) {
	str := func(v string) *parser.Node { return &parser.Node{Type: parser.TypeString, Value: v} }
	if got := describe(Compare(str("\xff"), str("\xfe"))); got != "" {
		t.Errorf("invalid UTF-8: got %q", got)
	}
	if got := describe(Compare(str("\xff"), str("\xef\xbf\xbd"))); got != "~$" {
		t.Errorf("invalid UTF-8 vs U+FFFD: got %q", got) // `"\ufffd"` vs `"` + U+FFFD + `"`
	}
	if got := describe(Compare(str("a"), str("b"))); got != "~$" {
		t.Errorf("got %q", got)
	}
	if got := describe(Compare(str("日本"), str("日本"))); got != "" {
		t.Errorf("got %q", got)
	}
}
