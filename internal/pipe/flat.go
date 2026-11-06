package pipe

import (
	"strings"

	"github.com/simota/jv/internal/parser"
)

// FlatFormatter prints one `path = value` line per leaf (and per empty
// container). The output is easy to grep, and every printed path can be fed
// back to `jv -p` or the TUI search to jump to the value.
type FlatFormatter struct {
	color Colorizer
}

func NewFlatFormatter(colorEnabled bool) *FlatFormatter {
	return &FlatFormatter{color: Colorizer{Enabled: colorEnabled}}
}

func (f *FlatFormatter) Format(root *parser.Node) string {
	var b strings.Builder
	for _, entry := range Flatten(root) {
		b.WriteString(f.color.Key(entry.Path))
		b.WriteString(" = ")
		b.WriteString(f.colorValue(entry.Node))
		b.WriteByte('\n')
	}
	return b.String()
}

func (f *FlatFormatter) colorValue(node *parser.Node) string {
	switch node.Type {
	case parser.TypeString:
		return f.color.String(node.StringValue())
	case parser.TypeNumber:
		return f.color.Number(node.StringValue())
	case parser.TypeBoolean:
		return f.color.Boolean(node.StringValue())
	case parser.TypeNull:
		return f.color.Null("null")
	case parser.TypeArray:
		return "[]"
	default:
		return "{}"
	}
}

// FlatEntry is a leaf value together with its path.
type FlatEntry struct {
	Path string
	Node *parser.Node
}

// Flatten lists every leaf value and empty container in document order.
// Paths are relative to root (root itself is `$`).
func Flatten(root *parser.Node) []FlatEntry {
	out := []FlatEntry{}
	var walk func(node *parser.Node, path string)
	walk = func(node *parser.Node, path string) {
		if len(node.Children) == 0 {
			out = append(out, FlatEntry{Path: path, Node: node})
			return
		}
		for _, child := range node.Children {
			walk(child, path+parser.ChildPathSegment(node, child))
		}
	}
	walk(root, "$")
	return out
}
