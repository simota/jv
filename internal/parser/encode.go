package parser

// Marshal encodes the node as JSON preserving key order. An empty indent
// produces compact output.
func Marshal(node *Node, indent string) string {
	return string(AppendJSON(nil, node, indent))
}

// AppendJSON appends Marshal(node, indent) to dst.
func AppendJSON(dst []byte, node *Node, indent string) []byte {
	return appendJSON(dst, node, indent, 0)
}

func appendJSON(dst []byte, node *Node, indent string, depth int) []byte {
	switch node.Type {
	case TypeObject, TypeArray:
		open, close := byte('{'), byte('}')
		if node.Type == TypeArray {
			open, close = '[', ']'
		}
		dst = append(dst, open)
		if len(node.Children) == 0 {
			return append(dst, close)
		}
		for i, child := range node.Children {
			if i > 0 {
				dst = append(dst, ',')
			}
			if indent != "" {
				dst = append(dst, '\n')
				dst = appendRepeat(dst, indent, depth+1)
			}
			if node.Type == TypeObject {
				dst = AppendQuoted(dst, child.Key)
				dst = append(dst, ':')
				if indent != "" {
					dst = append(dst, ' ')
				}
			}
			dst = appendJSON(dst, child, indent, depth+1)
		}
		if indent != "" {
			dst = append(dst, '\n')
			dst = appendRepeat(dst, indent, depth)
		}
		return append(dst, close)
	default:
		return AppendStringValue(dst, node)
	}
}

func appendRepeat(dst []byte, s string, n int) []byte {
	for ; n > 0; n-- {
		dst = append(dst, s...)
	}
	return dst
}
