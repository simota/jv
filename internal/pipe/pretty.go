package pipe

import (
	"github.com/simota/jv/internal/parser"
)

type PrettyFormatter struct {
	color   Colorizer
	compact bool
	raw     bool
}

func NewPrettyFormatter(colorEnabled bool) *PrettyFormatter {
	return &PrettyFormatter{color: Colorizer{Enabled: colorEnabled}}
}

// NewCompactFormatter renders each document on a single line.
func NewCompactFormatter(colorEnabled bool) *PrettyFormatter {
	return &PrettyFormatter{color: Colorizer{Enabled: colorEnabled}, compact: true}
}

// WithRaw makes top-level string values print without quotes or escaping
// (useful for shell scripts: `jv -r -p .name`).
func (f *PrettyFormatter) WithRaw(raw bool) *PrettyFormatter {
	f.raw = raw
	return f
}

func (f *PrettyFormatter) Format(root *parser.Node) string { return formatString(f, root) }

func (f *PrettyFormatter) FormatTo(out *Writer, root *parser.Node) {
	if f.raw && root.Type == parser.TypeString {
		out.str(root.RawString())
		out.byte('\n')
		out.maybeFlush()
		return
	}
	f.writeNode(out, root, 0)
	out.byte('\n')
	out.maybeFlush()
}

func (f *PrettyFormatter) writeNode(out *Writer, node *parser.Node, depth int) {
	switch node.Type {
	case parser.TypeObject:
		f.writeContainer(out, node, depth, '{', '}', true)
	case parser.TypeArray:
		f.writeContainer(out, node, depth, '[', ']', false)
	default:
		if !out.primitive(f.color, node) {
			out.str(node.StringValue())
		}
	}
}

func (f *PrettyFormatter) writeContainer(out *Writer, node *parser.Node, depth int, open, close byte, keys bool) {
	out.byte(open)
	if len(node.Children) == 0 {
		out.byte(close)
		return
	}
	for i, child := range node.Children {
		f.newline(out, depth+1)
		if keys {
			out.quotedKey(f.color, child.Key)
			out.byte(':')
			if !f.compact {
				out.byte(' ')
			}
		}
		f.writeNode(out, child, depth+1)
		if i < len(node.Children)-1 {
			out.byte(',')
		}
	}
	f.newline(out, depth)
	out.byte(close)
}

func (f *PrettyFormatter) newline(out *Writer, depth int) {
	out.maybeFlush()
	if f.compact {
		return
	}
	out.byte('\n')
	out.indent(2 * depth)
}
