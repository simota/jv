package tui

import (
	"strconv"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
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

// benchModel returns a fully expanded model over n records with a window.
func benchModel(b *testing.B, n int) Model {
	b.Helper()
	m := NewModel(benchTree(b, n), Options{Depth: 99})
	m.copyFn = func(string) error { return nil }
	next, _ := m.Update(tea.WindowSizeMsg{Width: 100, Height: 40})
	return next.(Model)
}

// BenchmarkNextMatch measures n on matches that are already visible.
func BenchmarkNextMatch(b *testing.B) {
	m := typeSearch(benchModel(b, 50000), "User-4")
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		m = press(m, "n")
	}
}

// BenchmarkSearchJump measures / text Enter including the jump to the match.
func BenchmarkSearchJump(b *testing.B) {
	m := benchModel(b, 50000)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		m = typeSearch(m, "User-49999")
	}
}

// BenchmarkExpandDepth measures the 1-9 keys (re-expand to a depth).
func BenchmarkExpandDepth(b *testing.B) {
	m := benchModel(b, 50000)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		m = press(m, "1", "3")
	}
}
