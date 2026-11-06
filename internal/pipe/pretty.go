package pipe

import (
	"bytes"
	"strings"

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

func (f *PrettyFormatter) Format(root *parser.Node) string {
	var buf bytes.Buffer
	if f.raw && root.Type == parser.TypeString {
		buf.WriteString(root.RawString())
		buf.WriteByte('\n')
		return buf.String()
	}
	f.writeNode(&buf, root, 0)
	buf.WriteByte('\n')
	return buf.String()
}

func (f *PrettyFormatter) writeNode(buf *bytes.Buffer, node *parser.Node, depth int) {
	switch node.Type {
	case parser.TypeObject:
		f.writeObject(buf, node, depth)
	case parser.TypeArray:
		f.writeArray(buf, node, depth)
	default:
		buf.WriteString(f.formatPrimitive(node))
	}
}

func (f *PrettyFormatter) writeObject(buf *bytes.Buffer, node *parser.Node, depth int) {
	buf.WriteString("{")
	if len(node.Children) == 0 {
		buf.WriteString("}")
		return
	}
	for i, child := range node.Children {
		f.newline(buf, depth+1)
		buf.WriteString(f.color.Key(parser.QuoteString(child.Key)))
		buf.WriteString(":")
		if !f.compact {
			buf.WriteString(" ")
		}
		f.writeNode(buf, child, depth+1)
		if i < len(node.Children)-1 {
			buf.WriteString(",")
		}
	}
	f.newline(buf, depth)
	buf.WriteString("}")
}

func (f *PrettyFormatter) writeArray(buf *bytes.Buffer, node *parser.Node, depth int) {
	buf.WriteString("[")
	if len(node.Children) == 0 {
		buf.WriteString("]")
		return
	}
	for i, child := range node.Children {
		f.newline(buf, depth+1)
		f.writeNode(buf, child, depth+1)
		if i < len(node.Children)-1 {
			buf.WriteString(",")
		}
	}
	f.newline(buf, depth)
	buf.WriteString("]")
}

func (f *PrettyFormatter) formatPrimitive(node *parser.Node) string {
	switch node.Type {
	case parser.TypeString:
		return f.color.String(node.StringValue())
	case parser.TypeNumber:
		return f.color.Number(node.StringValue())
	case parser.TypeBoolean:
		return f.color.Boolean(node.StringValue())
	case parser.TypeNull:
		return f.color.Null("null")
	default:
		return node.StringValue()
	}
}

func (f *PrettyFormatter) newline(buf *bytes.Buffer, depth int) {
	if f.compact {
		return
	}
	buf.WriteByte('\n')
	buf.WriteString(strings.Repeat("  ", depth))
}
