package pipe

import (
	"bytes"

	"github.com/simota/jv/internal/diff"
	"github.com/simota/jv/internal/parser"
)

// FormatDiff renders changes as one line per difference:
//
//	~ $.age: 30 -> 31
//	+ $.tags[3]: "new"
//	- $.legacy: {"a":1}
func FormatDiff(changes []diff.Change, colorEnabled bool) string {
	var buf bytes.Buffer
	out := NewWriter(&buf)
	FormatDiffTo(out, changes, colorEnabled)
	_ = out.Flush() // writes to a bytes.Buffer do not fail
	return buf.String()
}

// FormatDiffTo writes the text of FormatDiff to out.
func FormatDiffTo(out *Writer, changes []diff.Change, colorEnabled bool) {
	c := Colorizer{Enabled: colorEnabled}
	for _, ch := range changes {
		code := codeChanged
		switch ch.Kind {
		case diff.Added:
			code = codeAdded
		case diff.Removed:
			code = codeRemoved
		}
		out.open(c, code)
		out.str(ch.Kind.Symbol())
		out.byte(' ')
		out.str(ch.Path)
		out.str(": ")
		switch ch.Kind {
		case diff.Added:
			out.buf = parser.AppendJSON(out.buf, ch.New, "")
		case diff.Removed:
			out.buf = parser.AppendJSON(out.buf, ch.Old, "")
		default:
			out.buf = parser.AppendJSON(out.buf, ch.Old, "")
			out.str(" -> ")
			out.buf = parser.AppendJSON(out.buf, ch.New, "")
		}
		out.close(c)
		out.byte('\n')
		out.maybeFlush()
	}
}
