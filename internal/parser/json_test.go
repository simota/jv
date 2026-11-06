package parser

import (
	"errors"
	"strings"
	"testing"
)

func mustParse(t *testing.T, s string) *Node {
	t.Helper()
	n, err := Parse(strings.NewReader(s))
	if err != nil {
		t.Fatalf("parse %q: %v", s, err)
	}
	return n
}

func keys(n *Node) []string {
	out := []string{}
	for _, c := range n.Children {
		out = append(out, c.Key)
	}
	return out
}

func TestParsePreservesKeyOrder(t *testing.T) {
	n := mustParse(t, `{"z":1,"a":2,"m":{"y":1,"b":2}}`)
	if got := strings.Join(keys(n), ","); got != "z,a,m" {
		t.Fatalf("order = %s", got)
	}
	if got := strings.Join(keys(n.Children[2]), ","); got != "y,b" {
		t.Fatalf("nested order = %s", got)
	}
	SortKeys(n)
	if got := strings.Join(keys(n), ","); got != "a,m,z" {
		t.Fatalf("sorted = %s", got)
	}
}

func TestParseDuplicateKeysLastWins(t *testing.T) {
	n := mustParse(t, `{"a":1,"b":2,"a":3}`)
	if len(n.Children) != 2 || n.Children[0].Key != "a" || n.Children[0].Value != "3" {
		t.Fatalf("unexpected children: %+v", n.Children)
	}
	if n.Children[0].Parent != n {
		t.Fatal("parent not set")
	}
}

func TestParseTypesAndDepth(t *testing.T) {
	n := mustParse(t, `{"s":"x","n":1.50,"b":false,"z":null,"a":[{"k":[]}]}`)
	want := []NodeType{TypeString, TypeNumber, TypeBoolean, TypeNull, TypeArray}
	for i, w := range want {
		if n.Children[i].Type != w {
			t.Errorf("child %d type = %s want %s", i, n.Children[i].Type, w)
		}
	}
	if n.Children[1].StringValue() != "1.50" {
		t.Errorf("number not preserved verbatim: %s", n.Children[1].StringValue())
	}
	deep := n.Children[4].Children[0].Children[0]
	if deep.Depth != 3 || deep.Path() != "$.a[0].k" {
		t.Errorf("depth=%d path=%s", deep.Depth, deep.Path())
	}
}

func TestParseMultipleDocuments(t *testing.T) {
	docs, err := ParseAll(strings.NewReader("{\"a\":1}\n{\"a\":2}\n[3]"))
	if err != nil {
		t.Fatal(err)
	}
	if len(docs) != 3 {
		t.Fatalf("docs = %d", len(docs))
	}
	root := mustParse(t, "{\"a\":1}\n{\"a\":2}")
	if root.Type != TypeArray || len(root.Children) != 2 {
		t.Fatalf("wrapped root = %s %d", root.Type, len(root.Children))
	}
	leaf := root.Children[1].Children[0]
	if leaf.Path() != "$[1].a" || leaf.Depth != 2 {
		t.Fatalf("path=%s depth=%d", leaf.Path(), leaf.Depth)
	}
}

func TestParseErrorsHavePosition(t *testing.T) {
	cases := []struct {
		in         string
		line, col  int
		msgContain string
	}{
		{"{\n  \"a\": 1,\n  \"b\": }", 3, 8, "invalid character"},
		{`{"a":1,`, 1, 8, "unexpected end"},
		{"{\"a\":1}\n{oops}", 2, 2, "invalid character"},
	}
	for _, c := range cases {
		_, err := Parse(strings.NewReader(c.in))
		var se *SyntaxError
		if !errors.As(err, &se) {
			t.Fatalf("%q: expected SyntaxError, got %v", c.in, err)
		}
		if se.Line != c.line || se.Column != c.col || !strings.Contains(se.Error(), c.msgContain) {
			t.Errorf("%q: got %v (line %d col %d)", c.in, se, se.Line, se.Column)
		}
	}
	if _, err := Parse(strings.NewReader("   ")); err == nil {
		t.Error("expected error for empty input")
	}
}

func TestStringValueIsValidJSON(t *testing.T) {
	n := mustParse(t, `{"k":"a\u0000<b>\"日本"}`)
	got := n.Children[0].StringValue()
	if got != `"a\u0000<b>\"日本"` {
		t.Fatalf("StringValue = %s", got)
	}
	if n.Children[0].RawString() != "a\x00<b>\"日本" {
		t.Fatalf("RawString = %q", n.Children[0].RawString())
	}
}

func TestPathQuotesSpecialKeys(t *testing.T) {
	n := mustParse(t, `{"key with space":{"a-b":[1]}}`)
	leaf := n.Children[0].Children[0].Children[0]
	if got := leaf.Path(); got != `$["key with space"]["a-b"][0]` {
		t.Fatalf("path = %s", got)
	}
}

func TestMarshalRoundTrip(t *testing.T) {
	src := `{"z":[1,{"b":null,"a":true}],"e":{},"f":[],"s":"x<y"}`
	n := mustParse(t, src)
	if got := Marshal(n, ""); got != src {
		t.Fatalf("compact = %s", got)
	}
	pretty := Marshal(n, "  ")
	want := "{\n  \"z\": [\n    1,\n    {\n      \"b\": null,\n      \"a\": true\n    }\n  ],\n  \"e\": {},\n  \"f\": [],\n  \"s\": \"x<y\"\n}"
	if pretty != want {
		t.Fatalf("pretty =\n%s", pretty)
	}
}
