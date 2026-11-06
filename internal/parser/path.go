package parser

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
)

type segmentKind int

const (
	segKey segmentKind = iota
	segIndex
	segWildcard
)

type segment struct {
	kind  segmentKind
	key   string
	index int
}

// Query is a compiled path expression such as `$.users[0].name`,
// `.items[*].id`, `["key with space"]` or `list[-1]`.
type Query struct {
	expr     string
	segments []segment
}

func (q *Query) String() string { return q.expr }

// CompilePath parses a path expression. The syntax is the same one jv shows
// in its TUI footer (and copies with `Y`), plus a few conveniences:
//   - the leading `$` is optional (`.a.b`, `a.b` also work)
//   - `[*]` / `.*` match every child of an array or object
//   - negative indexes count from the end (`[-1]` is the last element)
func CompilePath(expr string) (*Query, error) {
	s := strings.TrimSpace(expr)
	q := &Query{expr: s}
	i := 0
	if strings.HasPrefix(s, "$") {
		i = 1
	}
	first := true
	for i < len(s) {
		c := s[i]
		switch {
		case c == '.':
			i++
			if i < len(s) && s[i] == '[' {
				// `.[0]` (jq style) is the same as `[0]`
				break
			}
			if i < len(s) && s[i] == '*' {
				q.segments = append(q.segments, segment{kind: segWildcard})
				i++
				break
			}
			if i < len(s) && s[i] == '"' {
				end, key, err := readQuoted(s, i)
				if err != nil {
					return nil, err
				}
				q.segments = append(q.segments, segment{kind: segKey, key: key})
				i = end
				break
			}
			end := scanIdent(s, i)
			if end == i {
				if i >= len(s) && len(q.segments) == 0 {
					// "." alone means the root
					break
				}
				return nil, fmt.Errorf("invalid path %q: expected key at position %d", expr, i+1)
			}
			q.segments = append(q.segments, segment{kind: segKey, key: s[i:end]})
			i = end
		case c == '[':
			i++
			if i >= len(s) {
				return nil, fmt.Errorf("invalid path %q: unterminated '['", expr)
			}
			switch s[i] {
			case '*':
				i++
				q.segments = append(q.segments, segment{kind: segWildcard})
			case '"':
				end, key, err := readQuoted(s, i)
				if err != nil {
					return nil, err
				}
				q.segments = append(q.segments, segment{kind: segKey, key: key})
				i = end
			default:
				end := strings.IndexByte(s[i:], ']')
				if end < 0 {
					return nil, fmt.Errorf("invalid path %q: unterminated '['", expr)
				}
				n, err := strconv.Atoi(strings.TrimSpace(s[i : i+end]))
				if err != nil {
					return nil, fmt.Errorf("invalid path %q: bad index %q", expr, s[i:i+end])
				}
				q.segments = append(q.segments, segment{kind: segIndex, index: n})
				i += end
			}
			if i >= len(s) || s[i] != ']' {
				return nil, fmt.Errorf("invalid path %q: expected ']'", expr)
			}
			i++
		case first && c != '$':
			// bare leading key: `a.b`
			end := scanIdent(s, i)
			if end == i {
				return nil, fmt.Errorf("invalid path %q: unexpected %q", expr, string(c))
			}
			q.segments = append(q.segments, segment{kind: segKey, key: s[i:end]})
			i = end
		default:
			return nil, fmt.Errorf("invalid path %q: unexpected %q at position %d", expr, string(c), i+1)
		}
		first = false
	}
	return q, nil
}

// scanIdent reads a bare key. Besides identifier characters it accepts `-`
// so that common kebab-case keys work without quoting.
func scanIdent(s string, i int) int {
	j := i
	for j < len(s) {
		c := s[j]
		if c == '.' || c == '[' || c == ']' || c == ' ' || c == '"' {
			break
		}
		j++
	}
	return j
}

func readQuoted(s string, start int) (int, string, error) {
	// find the closing quote honoring escapes
	j := start + 1
	for j < len(s) {
		if s[j] == '\\' {
			j += 2
			continue
		}
		if s[j] == '"' {
			var key string
			if err := json.Unmarshal([]byte(s[start:j+1]), &key); err != nil {
				return 0, "", fmt.Errorf("invalid quoted key %s: %v", s[start:j+1], err)
			}
			return j + 1, key, nil
		}
		j++
	}
	return 0, "", fmt.Errorf("invalid path %q: unterminated string", s)
}

// Find returns every node matched by the query, in document order.
func (q *Query) Find(root *Node) []*Node {
	current := []*Node{root}
	for _, seg := range q.segments {
		next := []*Node{}
		for _, node := range current {
			switch seg.kind {
			case segWildcard:
				next = append(next, node.Children...)
			case segKey:
				if node.Type != TypeObject {
					continue
				}
				for _, child := range node.Children {
					if child.Key == seg.key {
						next = append(next, child)
						break
					}
				}
			case segIndex:
				if node.Type != TypeArray {
					continue
				}
				idx := seg.index
				if idx < 0 {
					idx += len(node.Children)
				}
				if idx >= 0 && idx < len(node.Children) {
					next = append(next, node.Children[idx])
				}
			}
		}
		current = next
		if len(current) == 0 {
			break
		}
	}
	return current
}
