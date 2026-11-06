package parser

import (
	"encoding/json"
	"strconv"
	"unicode/utf8"
)

// maxNesting bounds recursion so hostile input cannot exhaust the stack
// (encoding/json uses the same limit).
const maxNesting = 10000

// scanner is a small single-pass JSON parser that builds Nodes directly,
// preserving key order. It is considerably faster than json.Decoder.Token
// and reports exact byte offsets for syntax errors.
type scanner struct {
	data []byte
	pos  int
}

func (s *scanner) errorAt(pos int, msg string) error {
	line, col := lineCol(s.data, int64(pos))
	return &SyntaxError{Msg: msg, Offset: int64(pos), Line: line, Column: col}
}

func (s *scanner) unexpected(context string) error {
	if s.pos >= len(s.data) {
		return s.errorAt(s.pos, "unexpected end of input")
	}
	r, _ := utf8.DecodeRune(s.data[s.pos:])
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
	node := &Node{Key: key, Parent: parent, Depth: depth}
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
		dup := -1
		if index != nil {
			if i, ok := index[key]; ok {
				dup = i
			}
		} else {
			for i, c := range node.Children {
				if c.Key == key {
					dup = i
					break
				}
			}
		}
		if dup >= 0 {
			node.Children[dup] = child
		} else {
			node.Children = append(node.Children, child)
			if index == nil && len(node.Children) > 16 {
				index = make(map[string]int, len(node.Children)*2)
				for i, c := range node.Children {
					index[c.Key] = i
				}
			} else if index != nil {
				index[key] = len(node.Children) - 1
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
	for i := 0; ; i++ {
		child, err := s.value(strconv.Itoa(i), node, depth+1)
		if err != nil {
			return err
		}
		node.Children = append(node.Children, child)
		s.skipWS()
		if s.pos >= len(s.data) {
			return s.unexpected("")
		}
		switch s.data[s.pos] {
		case ',':
			s.pos++
		case ']':
			s.pos++
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
			if !escaped && utf8.Valid(raw) {
				return string(raw), nil
			}
			var out string
			if err := json.Unmarshal(s.data[start:s.pos], &out); err != nil {
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
	return string(d[start:s.pos]), nil
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
