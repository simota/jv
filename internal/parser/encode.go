package parser

import (
	"bytes"
	"strings"
)

// Marshal encodes the node as JSON preserving key order. An empty indent
// produces compact output.
func Marshal(node *Node, indent string) string {
	var buf bytes.Buffer
	writeJSON(&buf, node, indent, 0)
	return buf.String()
}

func writeJSON(buf *bytes.Buffer, node *Node, indent string, depth int) {
	switch node.Type {
	case TypeObject, TypeArray:
		open, close := "{", "}"
		if node.Type == TypeArray {
			open, close = "[", "]"
		}
		buf.WriteString(open)
		if len(node.Children) == 0 {
			buf.WriteString(close)
			return
		}
		for i, child := range node.Children {
			if i > 0 {
				buf.WriteByte(',')
			}
			if indent != "" {
				buf.WriteByte('\n')
				buf.WriteString(strings.Repeat(indent, depth+1))
			}
			if node.Type == TypeObject {
				buf.WriteString(QuoteString(child.Key))
				buf.WriteByte(':')
				if indent != "" {
					buf.WriteByte(' ')
				}
			}
			writeJSON(buf, child, indent, depth+1)
		}
		if indent != "" {
			buf.WriteByte('\n')
			buf.WriteString(strings.Repeat(indent, depth))
		}
		buf.WriteString(close)
	default:
		buf.WriteString(node.StringValue())
	}
}
