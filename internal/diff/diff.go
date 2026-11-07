// Package diff computes structural differences between two JSON documents.
package diff

import (
	"math/big"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/simota/jv/internal/parser"
)

type Kind int

const (
	Added Kind = iota
	Removed
	Changed
)

func (k Kind) Symbol() string {
	switch k {
	case Added:
		return "+"
	case Removed:
		return "-"
	default:
		return "~"
	}
}

// Change is one difference, addressed by a jv path (`$.a[0].b`).
type Change struct {
	Kind Kind
	Path string
	Old  *parser.Node // nil for Added
	New  *parser.Node // nil for Removed
}

// Compare returns the differences between a and b in document order.
// Objects are compared by key (key order is ignored), arrays by index.
func Compare(a, b *parser.Node) []Change {
	changes := []Change{}
	path := make([]byte, 0, 128)
	path = append(path, '$')
	compare(a, b, path, &changes)
	return changes
}

// smallObject bounds the objects whose keys are matched by linear search
// instead of maps (len(a.Children)*len(b.Children)).
const smallObject = 64

// compare appends the differences below a and b. path is the path of a and
// b; it is turned into a string only when a change is recorded, and callers
// reuse its buffer for siblings.
func compare(a, b *parser.Node, path []byte, out *[]Change) {
	if a.Type != b.Type {
		*out = append(*out, Change{Kind: Changed, Path: string(path), Old: a, New: b})
		return
	}
	switch a.Type {
	case parser.TypeObject:
		compareObjects(a, b, path, out)
	case parser.TypeArray:
		n := len(a.Children)
		if len(b.Children) > n {
			n = len(b.Children)
		}
		for i := 0; i < n; i++ {
			switch {
			case i >= len(a.Children):
				bc := b.Children[i]
				*out = append(*out, Change{Kind: Added, Path: childPath(path, b, bc), New: bc})
			case i >= len(b.Children):
				ac := a.Children[i]
				*out = append(*out, Change{Kind: Removed, Path: childPath(path, a, ac), Old: ac})
			default:
				ac := a.Children[i]
				compare(ac, b.Children[i], parser.AppendChildPathSegment(path, a, ac), out)
			}
		}
	case parser.TypeNumber:
		if !numbersEqual(a.StringValue(), b.StringValue()) {
			*out = append(*out, Change{Kind: Changed, Path: string(path), Old: a, New: b})
		}
	case parser.TypeString:
		if !stringsEqual(a, b) {
			*out = append(*out, Change{Kind: Changed, Path: string(path), Old: a, New: b})
		}
	default:
		if a.StringValue() != b.StringValue() {
			*out = append(*out, Change{Kind: Changed, Path: string(path), Old: a, New: b})
		}
	}
}

func childPath(path []byte, parent, child *parser.Node) string {
	return string(parser.AppendChildPathSegment(path, parent, child))
}

func compareObjects(a, b *parser.Node, path []byte, out *[]Change) {
	if len(a.Children)*len(b.Children) <= smallObject {
		for _, ac := range a.Children {
			if bc := lastWithKey(b.Children, ac.Key); bc != nil {
				compare(ac, bc, parser.AppendChildPathSegment(path, a, ac), out)
			} else {
				*out = append(*out, Change{Kind: Removed, Path: childPath(path, a, ac), Old: ac})
			}
		}
		for _, bc := range b.Children {
			if lastWithKey(a.Children, bc.Key) == nil {
				*out = append(*out, Change{Kind: Added, Path: childPath(path, b, bc), New: bc})
			}
		}
		return
	}
	bIndex := make(map[string]*parser.Node, len(b.Children))
	for _, child := range b.Children {
		bIndex[child.Key] = child
	}
	seen := make(map[string]bool, len(a.Children))
	for _, ac := range a.Children {
		seen[ac.Key] = true
		if bc, ok := bIndex[ac.Key]; ok {
			compare(ac, bc, parser.AppendChildPathSegment(path, a, ac), out)
		} else {
			*out = append(*out, Change{Kind: Removed, Path: childPath(path, a, ac), Old: ac})
		}
	}
	for _, bc := range b.Children {
		if !seen[bc.Key] {
			*out = append(*out, Change{Kind: Added, Path: childPath(path, b, bc), New: bc})
		}
	}
}

// lastWithKey returns the last child with the given key (the one a map
// built in order would keep), or nil.
func lastWithKey(children []*parser.Node, key string) *parser.Node {
	for i := len(children) - 1; i >= 0; i-- {
		if children[i].Key == key {
			return children[i]
		}
	}
	return nil
}

// stringsEqual reports a.StringValue() == b.StringValue() without quoting
// both strings: quoting maps distinct valid UTF-8 strings to distinct
// literals, so only invalid UTF-8 (never produced by the parser) needs the
// literal comparison.
func stringsEqual(a, b *parser.Node) bool {
	as, ok1 := a.Value.(string)
	bs, ok2 := b.Value.(string)
	if ok1 && ok2 {
		if as == bs {
			return true
		}
		if utf8.ValidString(as) && utf8.ValidString(bs) {
			return false
		}
	}
	return a.StringValue() == b.StringValue()
}

// numbersEqual treats numerically equal literals (1, 1.0, 1e0) as equal.
// The comparison is exact: literals that differ in any significant digit are
// different, however many digits they have (a 64-bit float comparison would
// call 123456789012345678901 and 123456789012345678902 equal).
func numbersEqual(x, y string) bool {
	if x == y {
		return true
	}
	a, ok1 := parseDecimal(x)
	b, ok2 := parseDecimal(y)
	if !ok1 || !ok2 {
		return false
	}
	return a.negative == b.negative && a.digits == b.digits && a.exp.Cmp(b.exp) == 0
}

// decimal is a normalized number: sign * 0.digits * 10^exp, with no leading
// or trailing zeros in digits (zero has no digits and is unsigned).
type decimal struct {
	negative bool
	digits   string
	exp      *big.Int
}

// maxExponentDigits bounds the exponents (significant digits, leading zeros
// do not count) that are compared numerically; longer ones fall back to
// textual comparison.
const maxExponentDigits = 30

func parseDecimal(s string) (decimal, bool) {
	negative := false
	if strings.HasPrefix(s, "-") {
		negative = true
		s = s[1:]
	} else {
		s = strings.TrimPrefix(s, "+")
	}
	exp := new(big.Int)
	if i := strings.IndexAny(s, "eE"); i >= 0 {
		e := s[i+1:]
		if len(strings.TrimLeft(strings.TrimLeft(e, "+-"), "0")) > maxExponentDigits {
			return decimal{}, false
		}
		if _, ok := exp.SetString(e, 10); !ok {
			return decimal{}, false
		}
		s = s[:i]
	}
	intPart, fracPart, _ := strings.Cut(s, ".")
	if intPart == "" && fracPart == "" {
		return decimal{}, false
	}
	all := intPart + fracPart
	for i := 0; i < len(all); i++ {
		if all[i] < '0' || all[i] > '9' {
			return decimal{}, false
		}
	}
	trimmed := strings.TrimLeft(all, "0")
	lead := len(all) - len(trimmed)
	digits := strings.TrimRight(trimmed, "0")
	if digits == "" {
		return decimal{exp: new(big.Int)}, true
	}
	// value = 0.digits * 10^(len(intPart) - lead + exp)
	exp.Add(exp, big.NewInt(int64(len(intPart)-lead)))
	return decimal{negative: negative, digits: digits, exp: exp}, true
}

// Summary returns counts in a short human readable form.
func Summary(changes []Change) string {
	var added, removed, changed int
	for _, c := range changes {
		switch c.Kind {
		case Added:
			added++
		case Removed:
			removed++
		default:
			changed++
		}
	}
	parts := []string{}
	if changed > 0 {
		parts = append(parts, strconv.Itoa(changed)+" changed")
	}
	if added > 0 {
		parts = append(parts, strconv.Itoa(added)+" added")
	}
	if removed > 0 {
		parts = append(parts, strconv.Itoa(removed)+" removed")
	}
	if len(parts) == 0 {
		return "no differences"
	}
	return strings.Join(parts, ", ")
}
