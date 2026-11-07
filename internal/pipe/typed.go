package pipe

import (
	"strconv"

	"github.com/simota/jv/internal/parser"
)

type TypedFormatter struct {
	color Colorizer
}

func NewTypedFormatter(colorEnabled bool) *TypedFormatter {
	return &TypedFormatter{color: Colorizer{Enabled: colorEnabled}}
}

func (f *TypedFormatter) Format(root *parser.Node) string { return formatString(f, root) }

// FormatTo prints one line per node: tree prefix, label, value and type hint.
func (f *TypedFormatter) FormatTo(out *Writer, root *parser.Node) {
	prefix := make([]byte, 0, 64)
	f.walk(out, root, &prefix, true)
}

func (f *TypedFormatter) walk(out *Writer, node *parser.Node, prefix *[]byte, isLast bool) {
	out.buf = append(out.buf, *prefix...)
	if node.Parent != nil {
		if isLast {
			out.str("`- ")
		} else {
			out.str("|- ")
		}
	}

	f.writeLabel(out, node)
	// ": value" and "  type" are left out when they would be empty.
	mark := len(out.buf)
	out.str(": ")
	if valueStart := len(out.buf); !out.primitive(f.color, node) || len(out.buf) == valueStart {
		out.buf = out.buf[:mark]
	}
	mark = len(out.buf)
	out.str("  ")
	hintStart := len(out.buf)
	out.colored(f.color, codeHint, node.Type.String())
	if len(out.buf) == hintStart {
		out.buf = out.buf[:mark]
	}
	out.byte('\n')
	out.maybeFlush()

	if len(node.Children) == 0 {
		return
	}

	base := len(*prefix)
	if node.Parent != nil {
		if isLast {
			*prefix = append(*prefix, "   "...)
		} else {
			*prefix = append(*prefix, "|  "...)
		}
	}
	for i, child := range node.Children {
		f.walk(out, child, prefix, i == len(node.Children)-1)
	}
	*prefix = (*prefix)[:base]
}

func (f *TypedFormatter) writeLabel(out *Writer, node *parser.Node) {
	switch {
	case node.Parent == nil:
		out.colored(f.color, codeKey, "root")
	case node.Parent.Type == parser.TypeArray:
		out.open(f.color, codeKey)
		out.byte('[')
		out.str(node.Key)
		out.byte(']')
		out.close(f.color)
	default:
		out.quotedKey(f.color, node.Key)
	}
	f.containerHint(out, node)
}

func (f *TypedFormatter) containerHint(out *Writer, node *parser.Node) {
	var open, close byte
	switch node.Type {
	case parser.TypeObject:
		open, close = '{', '}'
	case parser.TypeArray:
		open, close = '[', ']'
	default:
		return
	}
	out.byte(' ')
	out.open(f.color, codeHint)
	out.byte(open)
	out.buf = strconv.AppendInt(out.buf, int64(len(node.Children)), 10)
	out.byte(close)
	out.close(f.color)
}
