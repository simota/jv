package parser

import (
	"errors"
	"fmt"
	"io"
	"slices"
	"strconv"
	"strings"
	"unicode/utf8"
)

// NodeType is the JSON type of a node. It is a small integer to keep Node
// compact (large documents have millions of nodes); String returns the type
// name ("object", "array", ...). The zero value is no type and prints as "".
type NodeType uint8

const (
	TypeObject NodeType = iota + 1
	TypeArray
	TypeString
	TypeNumber
	TypeBoolean
	TypeNull
)

var nodeTypeNames = [...]string{
	TypeObject:  "object",
	TypeArray:   "array",
	TypeString:  "string",
	TypeNumber:  "number",
	TypeBoolean: "boolean",
	TypeNull:    "null",
}

func (t NodeType) String() string {
	if int(t) < len(nodeTypeNames) {
		return nodeTypeNames[t]
	}
	return ""
}

// Node is one JSON value. The field order keeps the struct small.
type Node struct {
	Key      string
	Value    any
	Children []*Node
	Parent   *Node
	Depth    int32
	Type     NodeType
	Expanded bool
}

// SyntaxError describes invalid JSON input with a human friendly position.
type SyntaxError struct {
	Msg    string
	Offset int64
	Line   int
	Column int
}

func (e *SyntaxError) Error() string {
	return fmt.Sprintf("invalid JSON at line %d, column %d: %s", e.Line, e.Column, e.Msg)
}

// Parse reads a single JSON document. If the input contains several
// top-level values (e.g. JSON Lines), they are returned wrapped in an array.
func Parse(r io.Reader) (*Node, error) {
	docs, err := ParseAll(r)
	if err != nil {
		return nil, err
	}
	if len(docs) == 1 {
		return docs[0], nil
	}
	return WrapDocuments(docs), nil
}

// ParseAll reads every top-level JSON value from r (JSON, JSON Lines,
// or concatenated JSON) while preserving the original key order.
func ParseAll(r io.Reader) ([]*Node, error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return nil, err
	}
	return ParseBytes(data)
}

// ParseBytes is like ParseAll but operates on an in-memory buffer.
func ParseBytes(data []byte) ([]*Node, error) {
	return ParseString(string(data))
}

// ParseString is like ParseBytes but takes the input as a string, which
// avoids a copy when the caller already has one. The returned nodes do not
// refer to data.
func ParseString(data string) ([]*Node, error) {
	sc := &scanner{data: data}
	docs := []*Node{}
	for {
		sc.skipWS()
		if sc.pos >= len(sc.data) {
			break
		}
		node, err := sc.value("root", nil, 0)
		if err != nil {
			return nil, err
		}
		docs = append(docs, node)
	}
	if len(docs) == 0 {
		return nil, errors.New("no JSON value found in input")
	}
	return docs, nil
}

// WrapDocuments combines multiple documents into a single array root.
func WrapDocuments(docs []*Node) *Node {
	root := &Node{Key: "root", Type: TypeArray}
	for i, doc := range docs {
		doc.Key = strconv.Itoa(i)
		doc.Parent = root
		setDepth(doc, 1)
		root.Children = append(root.Children, doc)
	}
	return root
}

func setDepth(node *Node, depth int32) {
	node.Depth = depth
	for _, child := range node.Children {
		setDepth(child, depth+1)
	}
}

func lineCol(data string, offset int64) (int, int) {
	if offset > int64(len(data)) {
		offset = int64(len(data))
	}
	line, col := 1, 1
	for _, b := range []byte(data[:offset]) {
		if b == '\n' {
			line++
			col = 1
			continue
		}
		col++
	}
	return line, col
}

// SortKeys sorts object keys alphabetically in the whole subtree.
func SortKeys(node *Node) {
	if node.Type == TypeObject {
		slices.SortStableFunc(node.Children, func(a, b *Node) int {
			return strings.Compare(a.Key, b.Key)
		})
	}
	for _, child := range node.Children {
		SortKeys(child)
	}
}

func (n *Node) Path() string {
	if n.Parent == nil {
		return "$"
	}
	return n.Parent.Path() + ChildPathSegment(n.Parent, n)
}

// ChildPathSegment returns the path suffix that addresses child within parent
// (`.key`, `["odd key"]` or `[3]`).
func ChildPathSegment(parent, child *Node) string {
	if parent.Type == TypeArray {
		return "[" + child.Key + "]"
	}
	if isSimpleKey(child.Key) {
		return "." + child.Key
	}
	return "[" + QuoteString(child.Key) + "]"
}

// AppendChildPathSegment appends ChildPathSegment(parent, child) to dst.
func AppendChildPathSegment(dst []byte, parent, child *Node) []byte {
	if parent.Type == TypeArray {
		dst = append(dst, '[')
		dst = append(dst, child.Key...)
		return append(dst, ']')
	}
	if isSimpleKey(child.Key) {
		dst = append(dst, '.')
		return append(dst, child.Key...)
	}
	dst = append(dst, '[')
	dst = AppendQuoted(dst, child.Key)
	return append(dst, ']')
}

// isSimpleKey reports whether key matches ^[A-Za-z_][A-Za-z0-9_]*$, i.e.
// can be written as `.key` in a path.
func isSimpleKey(key string) bool {
	if key == "" {
		return false
	}
	for i := 0; i < len(key); i++ {
		c := key[i]
		if c == '_' || ('a' <= c && c <= 'z') || ('A' <= c && c <= 'Z') || (i > 0 && '0' <= c && c <= '9') {
			continue
		}
		return false
	}
	return true
}

// QuoteString returns s as a JSON string literal (without HTML escaping).
func QuoteString(s string) string {
	if !needsEscape(s) {
		return "\"" + s + "\""
	}
	return string(AppendQuoted(make([]byte, 0, len(s)+8), s))
}

func needsEscape(s string) bool {
	for i := 0; i < len(s); i++ {
		c := s[i]
		// control characters, quote, backslash, and any non-ASCII byte
		// (invalid UTF-8 and U+2028/U+2029 need escaping) take the slow path
		if c < 0x20 || c == '"' || c == '\\' || c >= utf8.RuneSelf {
			return true
		}
	}
	return false
}

// RawString returns the unquoted string content for string nodes and
// StringValue for everything else.
func (n *Node) RawString() string {
	if n.Type == TypeString {
		if v, ok := n.Value.(string); ok {
			return v
		}
	}
	return n.StringValue()
}

func (n *Node) StringValue() string {
	switch n.Type {
	case TypeString:
		if v, ok := n.Value.(string); ok {
			return QuoteString(v)
		}
		return QuoteString(fmt.Sprintf("%v", n.Value))
	case TypeNumber:
		if v, ok := n.Value.(string); ok {
			return v // what %v prints for a string, without fmt's overhead
		}
		return fmt.Sprintf("%v", n.Value)
	case TypeBoolean:
		if v, ok := n.Value.(bool); ok {
			return strconv.FormatBool(v)
		}
		return fmt.Sprintf("%v", n.Value)
	case TypeNull:
		return "null"
	default:
		return strings.TrimSpace(fmt.Sprintf("%v", n.Value))
	}
}

// AppendStringValue appends n.StringValue() to dst without building the
// intermediate string for the common cases.
func AppendStringValue(dst []byte, n *Node) []byte {
	switch n.Type {
	case TypeString:
		if v, ok := n.Value.(string); ok {
			return AppendQuoted(dst, v)
		}
	case TypeNumber:
		if v, ok := n.Value.(string); ok {
			return append(dst, v...)
		}
	case TypeBoolean:
		if v, ok := n.Value.(bool); ok {
			return strconv.AppendBool(dst, v)
		}
	case TypeNull:
		return append(dst, "null"...)
	}
	return append(dst, n.StringValue()...)
}

// Walk visits node and all descendants in document order. Returning false
// from fn skips the children of the visited node.
func Walk(node *Node, fn func(*Node) bool) {
	if !fn(node) {
		return
	}
	for _, child := range node.Children {
		Walk(child, fn)
	}
}
