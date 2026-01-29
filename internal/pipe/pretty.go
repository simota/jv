package pipe

import (
	"bytes"
	"strconv"
	"strings"

	"github.com/simota/jv/internal/parser"
)

type PrettyFormatter struct {
	color Colorizer
}

func NewPrettyFormatter(colorEnabled bool) *PrettyFormatter {
	return &PrettyFormatter{color: Colorizer{Enabled: colorEnabled}}
}

func (f *PrettyFormatter) Format(root *parser.Node) string {
	var buf bytes.Buffer
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
	buf.WriteByte('\n')
	indent := strings.Repeat("  ", depth+1)
	for i, child := range node.Children {
		buf.WriteString(indent)
		buf.WriteString(f.color.Key(strconv.Quote(child.Key)))
		buf.WriteString(": ")
		f.writeNode(buf, child, depth+1)
		if i < len(node.Children)-1 {
			buf.WriteString(",")
		}
		buf.WriteByte('\n')
	}
	buf.WriteString(strings.Repeat("  ", depth))
	buf.WriteString("}")
}

func (f *PrettyFormatter) writeArray(buf *bytes.Buffer, node *parser.Node, depth int) {
	buf.WriteString("[")
	if len(node.Children) == 0 {
		buf.WriteString("]")
		return
	}
	buf.WriteByte('\n')
	indent := strings.Repeat("  ", depth+1)
	for i, child := range node.Children {
		buf.WriteString(indent)
		f.writeNode(buf, child, depth+1)
		if i < len(node.Children)-1 {
			buf.WriteString(",")
		}
		buf.WriteByte('\n')
	}
	buf.WriteString(strings.Repeat("  ", depth))
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
