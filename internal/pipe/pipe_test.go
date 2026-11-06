package pipe

import (
	"strings"
	"testing"

	"github.com/simota/jv/internal/diff"
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

func TestPrettyKeepsOrderAndEscapes(t *testing.T) {
	got := NewPrettyFormatter(false).Format(parse(t, `{"b":"<\u0001>","a":[1,{}],"c":[]}`))
	want := "{\n  \"b\": \"<\\u0001>\",\n  \"a\": [\n    1,\n    {}\n  ],\n  \"c\": []\n}\n"
	if got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestCompactAndRaw(t *testing.T) {
	n := parse(t, `{"a": [1, 2], "b": {"c": "x"}}`)
	if got := NewCompactFormatter(false).Format(n); got != `{"a":[1,2],"b":{"c":"x"}}`+"\n" {
		t.Fatalf("compact = %q", got)
	}
	s := parse(t, `"line1\nline2"`)
	if got := NewPrettyFormatter(false).WithRaw(true).Format(s); got != "line1\nline2\n" {
		t.Fatalf("raw = %q", got)
	}
	if got := NewPrettyFormatter(false).WithRaw(true).Format(n.Children[0]); got != "[\n  1,\n  2\n]\n" {
		t.Fatalf("raw non-string = %q", got)
	}
}

func TestColorOutput(t *testing.T) {
	got := NewCompactFormatter(true).Format(parse(t, `{"a":null}`))
	if got != "{\x1b[36m\"a\"\x1b[0m:\x1b[90mnull\x1b[0m}\n" {
		t.Fatalf("color = %q", got)
	}
}

func TestFlat(t *testing.T) {
	got := NewFlatFormatter(false).Format(parse(t, `{"a":{"b":[1,"x"]},"odd key":{},"e":[],"n":null}`))
	want := strings.Join([]string{
		`$.a.b[0] = 1`,
		`$.a.b[1] = "x"`,
		`$["odd key"] = {}`,
		`$.e = []`,
		`$.n = null`,
	}, "\n") + "\n"
	if got != want {
		t.Fatalf("got:\n%s", got)
	}
	if got := NewFlatFormatter(false).Format(parse(t, `5`)); got != "$ = 5\n" {
		t.Fatalf("scalar root = %q", got)
	}
}

func TestSchemaMergesArrayElements(t *testing.T) {
	got := NewSchemaFormatter(false).Format(parse(t, `{
		"items": [{"id": 1, "name": "a"}, {"id": 2, "name": null, "extra": true}],
		"mixed": [1, "x", [true]],
		"empty": [],
		"maybe": [{"x": 1}, null]
	}`))
	want := `{
  "items": [
    {
      "id": number,
      "name": string | null,
      "extra"?: boolean
    }
  ],
  "mixed": [[boolean] | string | number],
  "empty": [],
  "maybe": [
    {
      "x": number
    } | null
  ]
}
`
	if got != want {
		t.Fatalf("got:\n%s\nwant:\n%s", got, want)
	}
}

func TestSchemaFormatManyMergesDocuments(t *testing.T) {
	docs, err := parser.ParseBytes([]byte("{\"a\":1}\n{\"a\":\"x\",\"b\":2}"))
	if err != nil {
		t.Fatal(err)
	}
	got := NewSchemaFormatter(false).FormatMany(docs)
	want := "{\n  \"a\": string | number,\n  \"b\"?: number\n}\n"
	if got != want {
		t.Fatalf("got:\n%s", got)
	}
}

func TestTypedFormatter(t *testing.T) {
	got := NewTypedFormatter(false).Format(parse(t, `{"a":[1],"b c":"x"}`))
	want := "root {2}  object\n|- \"a\" [1]  array\n|  `- [0]: 1  number\n`- \"b c\": \"x\"  string\n"
	if got != want {
		t.Fatalf("got:\n%s", got)
	}
}

func TestFormatDiff(t *testing.T) {
	changes := diff.Compare(parse(t, `{"a":1,"b":[1]}`), parse(t, `{"a":2,"c":true}`))
	got := FormatDiff(changes, false)
	want := "~ $.a: 1 -> 2\n- $.b: [1]\n+ $.c: true\n"
	if got != want {
		t.Fatalf("got:\n%s", got)
	}
}
