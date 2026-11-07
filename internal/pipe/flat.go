package pipe

import (
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

func (f *FlatFormatter) Format(root *parser.Node) string { return formatString(f, root) }

// FormatTo prints the entries of Flatten(root). The path is kept in one
// buffer that grows and shrinks with the walk instead of being built as a new
// string for every node.
func (f *FlatFormatter) FormatTo(out *Writer, root *parser.Node) {
	path := make([]byte, 0, 128)
	path = append(path, '$')
	f.walk(out, root, path)
}

func (f *FlatFormatter) walk(out *Writer, node *parser.Node, path []byte) {
	if len(node.Children) == 0 {
		out.open(f.color, codeKey)
		out.buf = append(out.buf, path...)
		out.close(f.color)
		out.str(" = ")
		f.writeValue(out, node)
		out.byte('\n')
		out.maybeFlush()
		return
	}
	for _, child := range node.Children {
		f.walk(out, child, parser.AppendChildPathSegment(path, node, child))
	}
}

func (f *FlatFormatter) writeValue(out *Writer, node *parser.Node) {
	if out.primitive(f.color, node) {
		return
	}
	if node.Type == parser.TypeArray {
		out.str("[]")
	} else {
		out.str("{}")
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
