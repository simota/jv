package pipe

import (
	"bytes"
	"strconv"
	"strings"

	"github.com/simota/jv/internal/parser"
)

type SchemaFormatter struct {
	color Colorizer
}

func NewSchemaFormatter(colorEnabled bool) *SchemaFormatter {
	return &SchemaFormatter{color: Colorizer{Enabled: colorEnabled}}
}

func (f *SchemaFormatter) Format(root *parser.Node) string {
	var buf bytes.Buffer
	f.writeSchema(&buf, root, 0)
	buf.WriteByte('\n')
	return buf.String()
}

func (f *SchemaFormatter) writeSchema(buf *bytes.Buffer, node *parser.Node, depth int) {
	switch node.Type {
	case parser.TypeObject:
		f.writeSchemaObject(buf, node, depth)
	case parser.TypeArray:
		f.writeSchemaArray(buf, node, depth)
	default:
		buf.WriteString(f.color.TypeHint(string(node.Type)))
	}
}

func (f *SchemaFormatter) writeSchemaObject(buf *bytes.Buffer, node *parser.Node, depth int) {
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
		f.writeSchema(buf, child, depth+1)
		if i < len(node.Children)-1 {
			buf.WriteString(",")
		}
		buf.WriteByte('\n')
	}
	buf.WriteString(strings.Repeat("  ", depth))
	buf.WriteString("}")
}

func (f *SchemaFormatter) writeSchemaArray(buf *bytes.Buffer, node *parser.Node, depth int) {
	if len(node.Children) == 0 {
		buf.WriteString("[]")
		return
	}
	buf.WriteString("[")
	f.writeSchema(buf, node.Children[0], depth+1)
	buf.WriteString("]")
}
