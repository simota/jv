package parser

import (
	"encoding/json"
	"strconv"
	"strings"
	"unicode/utf8"
)

// maxNesting bounds recursion so hostile input cannot exhaust the stack
// (encoding/json uses the same limit).
const maxNesting = 10000

// scanner is a small single-pass JSON parser that builds Nodes directly,
// preserving key order. It is considerably faster than json.Decoder.Token
// and reports exact byte offsets for syntax errors.
//
// Allocation is the dominant cost of building a large tree, so the scanner
// keeps it low: keys, strings and numbers are copied into shared string
// chunks (the input itself is not retained), nodes come from chunks,
// children are collected on a shared stack and moved into an exactly sized
// slice when their container closes, and array index keys are shared.
type scanner struct {
	data string
	pos  int

	nodes   []Node   // current chunk of preallocated nodes
	stack   []*Node  // children of the containers being parsed
	indexes []string // indexes[i] == strconv.Itoa(i)
	strs    strings.Builder
	scratch []byte // unquoting buffer
}

// String chunks grow up to maxStringChunk; longer strings get their own
// allocation.
const (
	minStringChunk = 1 << 10
	maxStringChunk = 64 << 10
	ownAllocation  = maxStringChunk / 8
)

// reserve makes room for n more bytes in the current string chunk.
func (s *scanner) reserve(n int) {
	if s.strs.Cap()-s.strs.Len() >= n {
		return
	}
	size := 2 * s.strs.Cap()
	if size < minStringChunk {
		size = minStringChunk
	} else if size > maxStringChunk {
		size = maxStringChunk
	}
	// Strings handed out earlier keep the old chunk alive; a new Builder
	// never moves them.
	s.strs = strings.Builder{}
	s.strs.Grow(size)
}

// intern returns a copy of v that lives in a shared string chunk. Bytes
// written to a Builder within its capacity are never moved or modified, so
// substrings of its String stay valid.
func (s *scanner) intern(v string) string {
	if len(v) == 0 {
		return ""
	}
	if len(v) > ownAllocation {
		return strings.Clone(v)
	}
	s.reserve(len(v))
	start := s.strs.Len()
	s.strs.WriteString(v)
	return s.strs.String()[start:]
}

// internBytes is intern for a byte slice.
func (s *scanner) internBytes(v []byte) string {
	if len(v) == 0 {
		return ""
	}
	if len(v) > ownAllocation {
		return string(v)
	}
	s.reserve(len(v))
	start := s.strs.Len()
	s.strs.Write(v)
	return s.strs.String()[start:]
}

// maxNodeChunk bounds the size of one node chunk. Chunks start small so that
// tiny documents do not pay for a large allocation.
const maxNodeChunk = 1024

func (s *scanner) newNode() *Node {
	if len(s.nodes) == 0 {
		n := 2 * cap(s.nodes)
		if n < 16 {
			n = 16
		} else if n > maxNodeChunk {
			n = maxNodeChunk
		}
		s.nodes = make([]Node, n)
	}
	node := &s.nodes[0]
	s.nodes = s.nodes[1:]
	return node
}

// indexKey returns strconv.Itoa(i), sharing the strings between arrays.
func (s *scanner) indexKey(i int) string {
	if i < len(s.indexes) {
		return s.indexes[i]
	}
	// Extend the table in batches whose digits share one allocation.
	end := 2 * len(s.indexes)
	if end < 64 {
		end = 64
	}
	if end <= i {
		end = i + 1
	}
	var b strings.Builder
	b.Grow((end - len(s.indexes)) * len(strconv.Itoa(end)))
	ends := make([]int, 0, end-len(s.indexes))
	for j := len(s.indexes); j < end; j++ {
		b.WriteString(strconv.Itoa(j))
		ends = append(ends, b.Len())
	}
	digits := b.String()
	start := 0
	for _, e := range ends {
		s.indexes = append(s.indexes, digits[start:e])
		start = e
	}
	return s.indexes[i]
}

// closeChildren moves the children collected since base into node.
func (s *scanner) closeChildren(node *Node, base int) {
	if n := len(s.stack) - base; n > 0 {
		node.Children = make([]*Node, n)
		copy(node.Children, s.stack[base:])
		clear(s.stack[base:])
		s.stack = s.stack[:base]
	}
}

func (s *scanner) errorAt(pos int, msg string) error {
	line, col := lineCol(s.data, int64(pos))
	return &SyntaxError{Msg: msg, Offset: int64(pos), Line: line, Column: col}
}

func (s *scanner) unexpected(context string) error {
	if s.pos >= len(s.data) {
		return s.errorAt(s.pos, "unexpected end of input")
	}
	r, _ := utf8.DecodeRuneInString(s.data[s.pos:])
	return s.errorAt(s.pos, "invalid character "+quoteChar(r)+" "+context)
}

func quoteChar(r rune) string {
	if r == '\'' {
		return `'\''`
	}
	if r == '"' {
		return `'"'`
	}
	q := strconv.Quote(string(r))
	return "'" + q[1:len(q)-1] + "'"
}

func (s *scanner) skipWS() {
	for s.pos < len(s.data) {
		switch s.data[s.pos] {
		case ' ', '\t', '\n', '\r':
			s.pos++
		default:
			return
		}
	}
}

func (s *scanner) value(key string, parent *Node, depth int) (*Node, error) {
	s.skipWS()
	if s.pos >= len(s.data) {
		return nil, s.unexpected("")
	}
	node := s.newNode()
	node.Key, node.Parent, node.Depth = key, parent, int32(depth)
	switch c := s.data[s.pos]; {
	case c == '{':
		if depth >= maxNesting {
			return nil, s.errorAt(s.pos, "exceeded max nesting depth")
		}
		node.Type = TypeObject
		return node, s.object(node, depth)
	case c == '[':
		if depth >= maxNesting {
			return nil, s.errorAt(s.pos, "exceeded max nesting depth")
		}
		node.Type = TypeArray
		return node, s.array(node, depth)
	case c == '"':
		str, err := s.str()
		if err != nil {
			return nil, err
		}
		node.Type = TypeString
		node.Value = str
	case c == '-' || (c >= '0' && c <= '9'):
		num, err := s.number()
		if err != nil {
			return nil, err
		}
		node.Type = TypeNumber
		node.Value = num
	case c == 't':
		if err := s.literal("true"); err != nil {
			return nil, err
		}
		node.Type = TypeBoolean
		node.Value = true
	case c == 'f':
		if err := s.literal("false"); err != nil {
			return nil, err
		}
		node.Type = TypeBoolean
		node.Value = false
	case c == 'n':
		if err := s.literal("null"); err != nil {
			return nil, err
		}
		node.Type = TypeNull
	default:
		return nil, s.unexpected("looking for beginning of value")
	}
	return node, nil
}

func (s *scanner) object(node *Node, depth int) error {
	s.pos++ // '{'
	s.skipWS()
	if s.pos < len(s.data) && s.data[s.pos] == '}' {
		s.pos++
		return nil
	}
	base := len(s.stack)
	var index map[string]int
	for {
		s.skipWS()
		if s.pos >= len(s.data) || s.data[s.pos] != '"' {
			return s.unexpected("looking for beginning of object key string")
		}
		key, err := s.str()
		if err != nil {
			return err
		}
		s.skipWS()
		if s.pos >= len(s.data) || s.data[s.pos] != ':' {
			return s.unexpected("after object key")
		}
		s.pos++
		child, err := s.value(key, node, depth+1)
		if err != nil {
			return err
		}
		// Duplicate keys: last value wins (as in encoding/json), keeping the
		// position of the first occurrence.
		children := s.stack[base:]
		dup := -1
		if index != nil {
			if i, ok := index[key]; ok {
				dup = i
			}
		} else {
			for i, c := range children {
				if c.Key == key {
					dup = i
					break
				}
			}
		}
		if dup >= 0 {
			children[dup] = child
		} else {
			s.stack = append(s.stack, child)
			if n := len(s.stack) - base; index == nil && n > 16 {
				index = make(map[string]int, n*2)
				for i, c := range s.stack[base:] {
					index[c.Key] = i
				}
			} else if index != nil {
				index[key] = n - 1
			}
		}
		s.skipWS()
		if s.pos >= len(s.data) {
			return s.unexpected("")
		}
		switch s.data[s.pos] {
		case ',':
			s.pos++
		case '}':
			s.pos++
			s.closeChildren(node, base)
			return nil
		default:
			return s.unexpected("after object key:value pair")
		}
	}
}

func (s *scanner) array(node *Node, depth int) error {
	s.pos++ // '['
	s.skipWS()
	if s.pos < len(s.data) && s.data[s.pos] == ']' {
		s.pos++
		return nil
	}
	base := len(s.stack)
	for i := 0; ; i++ {
		child, err := s.value(s.indexKey(i), node, depth+1)
		if err != nil {
			return err
		}
		s.stack = append(s.stack, child)
		s.skipWS()
		if s.pos >= len(s.data) {
			return s.unexpected("")
		}
		switch s.data[s.pos] {
		case ',':
			s.pos++
		case ']':
			s.pos++
			s.closeChildren(node, base)
			return nil
		default:
			return s.unexpected("after array element")
		}
	}
}

func (s *scanner) str() (string, error) {
	start := s.pos
	s.pos++ // opening quote
	escaped := false
	for s.pos < len(s.data) {
		c := s.data[s.pos]
		switch {
		case c == '"':
			s.pos++
			raw := s.data[start+1 : s.pos-1]
			if !escaped && utf8.ValidString(raw) {
				return s.intern(raw), nil
			}
			if out, ok := appendUnquoted(s.scratch[:0], raw); ok {
				s.scratch = out
				return s.internBytes(out), nil
			}
			// Invalid literal: let encoding/json describe the problem.
			var out string
			if err := json.Unmarshal([]byte(s.data[start:s.pos]), &out); err != nil {
				return "", s.errorAt(start, "invalid string literal: "+err.Error())
			}
			return out, nil
		case c == '\\':
			escaped = true
			s.pos += 2
		case c < 0x20:
			return "", s.unexpected("in string literal")
		default:
			s.pos++
		}
	}
	return "", s.errorAt(len(s.data), "unexpected end of input")
}

func (s *scanner) number() (string, error) {
	start := s.pos
	d := s.data
	if s.pos < len(d) && d[s.pos] == '-' {
		s.pos++
	}
	digits := func() int {
		n := 0
		for s.pos < len(d) && d[s.pos] >= '0' && d[s.pos] <= '9' {
			s.pos++
			n++
		}
		return n
	}
	if s.pos < len(d) && d[s.pos] == '0' {
		s.pos++
	} else if digits() == 0 {
		return "", s.unexpected("in numeric literal")
	}
	if s.pos < len(d) && d[s.pos] == '.' {
		s.pos++
		if digits() == 0 {
			return "", s.unexpected("after decimal point in numeric literal")
		}
	}
	if s.pos < len(d) && (d[s.pos] == 'e' || d[s.pos] == 'E') {
		s.pos++
		if s.pos < len(d) && (d[s.pos] == '+' || d[s.pos] == '-') {
			s.pos++
		}
		if digits() == 0 {
			return "", s.unexpected("in exponent of numeric literal")
		}
	}
	return s.intern(d[start:s.pos]), nil
}

func (s *scanner) literal(word string) error {
	for i := 0; i < len(word); i++ {
		if s.pos >= len(s.data) || s.data[s.pos] != word[i] {
			return s.unexpected("in literal " + word + " (expecting '" + string(word[i]) + "')")
		}
		s.pos++
	}
	return nil
}
