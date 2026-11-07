package tui

import (
	"strconv"
	"strings"
	"testing"

	"github.com/simota/jv/internal/parser"
)

func benchTree(b *testing.B, n int) *parser.Node {
	b.Helper()
	var sb strings.Builder
	sb.WriteByte('[')
	for i := 0; i < n; i++ {
		if i > 0 {
			sb.WriteByte(',')
		}
		sb.WriteString(`{"id":` + strconv.Itoa(i) + `,"name":"User-` + strconv.Itoa(i) + `","City":"Tōkyō ` + strconv.Itoa(i%30) + `","tags":["a","B"]}`)
	}
	sb.WriteByte(']')
	docs, err := parser.ParseString(sb.String())
	if err != nil {
		b.Fatal(err)
	}
	return docs[0]
}

// BenchmarkSearch measures a text search over the whole tree (keys and
// values, collapsed parts included).
func BenchmarkSearch(b *testing.B) {
	m := NewModel(benchTree(b, 50000), Options{Depth: 2})
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := m.findMatches("user-49"); err != nil {
			b.Fatal(err)
		}
	}
}

// BenchmarkLayout measures rebuilding the visible rows of a fully expanded tree.
func BenchmarkLayout(b *testing.B) {
	root := benchTree(b, 50000)
	applyExpandDepth(root, 99)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		layout(root)
	}
}
