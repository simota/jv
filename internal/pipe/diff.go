package pipe

import (
	"strings"

	"github.com/simota/jv/internal/diff"
	"github.com/simota/jv/internal/parser"
)

// FormatDiff renders changes as one line per difference:
//
//	~ $.age: 30 -> 31
//	+ $.tags[3]: "new"
//	- $.legacy: {"a":1}
func FormatDiff(changes []diff.Change, colorEnabled bool) string {
	c := Colorizer{Enabled: colorEnabled}
	var b strings.Builder
	for _, ch := range changes {
		line := ch.Kind.Symbol() + " " + ch.Path + ": "
		switch ch.Kind {
		case diff.Added:
			line += parser.Marshal(ch.New, "")
			b.WriteString(c.wrap(32, line))
		case diff.Removed:
			line += parser.Marshal(ch.Old, "")
			b.WriteString(c.wrap(31, line))
		default:
			line += parser.Marshal(ch.Old, "") + " -> " + parser.Marshal(ch.New, "")
			b.WriteString(c.wrap(33, line))
		}
		b.WriteByte('\n')
	}
	return b.String()
}
