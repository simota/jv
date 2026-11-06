// Package diff computes structural differences between two JSON documents.
package diff

import (
	"math/big"
	"strconv"
	"strings"

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
	compare(a, b, "$", &changes)
	return changes
}

func compare(a, b *parser.Node, path string, out *[]Change) {
	if a.Type != b.Type {
		*out = append(*out, Change{Kind: Changed, Path: path, Old: a, New: b})
		return
	}
	switch a.Type {
	case parser.TypeObject:
		bIndex := make(map[string]*parser.Node, len(b.Children))
		for _, child := range b.Children {
			bIndex[child.Key] = child
		}
		seen := make(map[string]bool, len(a.Children))
		for _, ac := range a.Children {
			seen[ac.Key] = true
			childPath := path + parser.ChildPathSegment(a, ac)
			if bc, ok := bIndex[ac.Key]; ok {
				compare(ac, bc, childPath, out)
			} else {
				*out = append(*out, Change{Kind: Removed, Path: childPath, Old: ac})
			}
		}
		for _, bc := range b.Children {
			if !seen[bc.Key] {
				*out = append(*out, Change{Kind: Added, Path: path + parser.ChildPathSegment(b, bc), New: bc})
			}
		}
	case parser.TypeArray:
		n := len(a.Children)
		if len(b.Children) > n {
			n = len(b.Children)
		}
		for i := 0; i < n; i++ {
			switch {
			case i >= len(a.Children):
				bc := b.Children[i]
				*out = append(*out, Change{Kind: Added, Path: path + parser.ChildPathSegment(b, bc), New: bc})
			case i >= len(b.Children):
				ac := a.Children[i]
				*out = append(*out, Change{Kind: Removed, Path: path + parser.ChildPathSegment(a, ac), Old: ac})
			default:
				ac := a.Children[i]
				compare(ac, b.Children[i], path+parser.ChildPathSegment(a, ac), out)
			}
		}
	case parser.TypeNumber:
		if !numbersEqual(a.StringValue(), b.StringValue()) {
			*out = append(*out, Change{Kind: Changed, Path: path, Old: a, New: b})
		}
	default:
		if a.StringValue() != b.StringValue() {
			*out = append(*out, Change{Kind: Changed, Path: path, Old: a, New: b})
		}
	}
}

// numbersEqual treats numerically equal literals (1, 1.0, 1e0) as equal.
func numbersEqual(x, y string) bool {
	if x == y {
		return true
	}
	fx, ok1 := new(big.Float).SetString(x)
	fy, ok2 := new(big.Float).SetString(y)
	if !ok1 || !ok2 {
		return false
	}
	return fx.Cmp(fy) == 0
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
