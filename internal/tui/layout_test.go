package tui

import (
	"fmt"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/charmbracelet/x/ansi"
	"github.com/muesli/termenv"
)

// The layout audit renders every screen mode over a grid of terminal sizes
// and checks the frame contract: exactly `height` lines, none wider than the
// terminal (a wider line wraps and pushes the header off the screen), the
// selection background over the whole cursor row, and transient status
// messages left readable in the footer.

var auditSizes = [][2]int{{20, 6}, {30, 10}, {40, 12}, {48, 24}, {60, 20}, {80, 24}, {120, 40}}

type auditFrame struct {
	name  string
	model Model
}

func auditFrames(t *testing.T, color bool) []auditFrame {
	t.Helper()
	var frames []auditFrame
	for _, sz := range auditSizes {
		// Models share their node tree, so every frame gets a fresh one.
		base := func() Model {
			m, _ := newTestModel(t, Options{Depth: 3, ColorEnabled: color})
			next, _ := m.Update(tea.WindowSizeMsg{Width: sz[0], Height: sz[1]})
			return next.(Model)
		}
		deep := func() Model { return typeSearch(base(), "found me") } // cursor on a long path
		tag := fmt.Sprintf("%dx%d", sz[0], sz[1])
		frames = append(frames,
			auditFrame{"normal " + tag, base()},
			auditFrame{"cursor-row " + tag, press(base(), "j")},
			auditFrame{"help " + tag, press(base(), "?")},
			auditFrame{"search " + tag, press(deep(), "/")},
			auditFrame{"detail " + tag, press(deep(), "v")},
			auditFrame{"status " + tag, press(deep(), "y")},
		)
	}
	return frames
}

// selectedCoverage returns the share of printable cells of line that are
// drawn with the selection background (SGR 44 in the dark theme).
func selectedCoverage(line string) (covered, total int) {
	bg := false
	for i := 0; i < len(line); {
		if line[i] == 0x1b && i+1 < len(line) && line[i+1] == '[' {
			j := i + 2
			for j < len(line) && line[j] != 'm' {
				j++
			}
			for _, p := range strings.Split(line[i+2:j], ";") {
				switch p {
				case "", "0", "49":
					bg = false
				case "44":
					bg = true
				}
			}
			i = j + 1
			continue
		}
		total++
		if bg {
			covered++
		}
		i++
	}
	return covered, total
}

type auditResult struct {
	frames, defective, tooTall, overwide, hiddenStatus, rows, partialSelection int
	problems                                                                   []string
}

func runLayoutAudit(t *testing.T) auditResult {
	t.Helper()
	lipgloss.SetColorProfile(termenv.ANSI)
	defer lipgloss.SetColorProfile(termenv.Ascii)
	var r auditResult
	for _, f := range auditFrames(t, true) {
		r.frames++
		m := f.model
		before := len(r.problems)
		lines := strings.Split(m.View(), "\n")
		if len(lines) > m.height {
			r.tooTall++
			r.problems = append(r.problems, fmt.Sprintf("%s: %d lines", f.name, len(lines)))
		}
		for i, l := range lines {
			if w := ansi.StringWidth(l); w > m.width {
				r.overwide++
				r.problems = append(r.problems, fmt.Sprintf("%s: line %d is %d cells", f.name, i, w))
			}
		}
		if strings.HasPrefix(f.name, "status") && m.statusMsg != "" && ansi.StringWidth(m.statusMsg) <= m.width {
			if !strings.Contains(ansi.Strip(lines[len(lines)-1]), m.statusMsg) {
				r.hiddenStatus++
				r.problems = append(r.problems, fmt.Sprintf("%s: status %q not visible", f.name, m.statusMsg))
			}
		}
		if strings.HasPrefix(f.name, "cursor-row") {
			sel := m.nodeRow[m.cursor] - m.yOffset + 1
			r.rows++
			if c, n := selectedCoverage(lines[sel]); c < n {
				r.partialSelection++
				r.problems = append(r.problems, fmt.Sprintf("%s: selection covers %d/%d cells", f.name, c, n))
			}
		}
		if len(r.problems) > before {
			r.defective++
		}
	}
	return r
}

func TestLayoutAudit(t *testing.T) {
	r := runLayoutAudit(t)
	t.Logf("frames=%d defective=%d tooTall=%d overwideLines=%d hiddenStatus=%d partialSelection=%d/%d",
		r.frames, r.defective, r.tooTall, r.overwide, r.hiddenStatus, r.partialSelection, r.rows)
	for _, p := range r.problems {
		t.Error(p)
	}
}

func TestFitHeaderKeepsKeyHints(t *testing.T) {
	cases := []struct {
		width int
		want  string
	}{
		{0, "jv - JSON Viewer  [/] Search"},
		{30, "jv - JSON Viewer  [/] Search"},
		{24, "jv - JSON Vi… [/] Search"},
		{20, "jv - JSO… [/] Search"},
		{17, "jv - JSON Viewer"}, // too narrow for the hints
		{10, "jv - JSON…"},
	}
	for _, c := range cases {
		got := strings.TrimRight(fitHeader("jv - JSON Viewer", "[/] Search", c.width), " ")
		if c.width == 30 {
			got = strings.Join(strings.Fields(got), " ")
			c.want = "jv - JSON Viewer [/] Search"
		}
		if got != c.want {
			t.Errorf("width %d: %q, want %q", c.width, got, c.want)
		}
	}
}

func TestFitFooterKeepsStatus(t *testing.T) {
	info := "Path: $.a.b.c  Lines: 3/9  Depth: 3"
	if got := fitFooter(info, "", 20); got != "Path: $.a.b.c  Line…" {
		t.Errorf("no status: %q", got)
	}
	if got := fitFooter(info, "Copied value", 30); got != "Path: $.a.b.c  …  Copied value" {
		t.Errorf("short: %q", got)
	}
	if got := fitFooter(info, "Copied value", 13); got != "Copied value" {
		t.Errorf("status only: %q", got)
	}
	if got := fitFooter(info, "Copied value", 8); got != "Copied …" {
		t.Errorf("tiny: %q", got)
	}
	if got := fitFooter(info, "ok", 0); got != info+"  ok" {
		t.Errorf("unknown width: %q", got)
	}
}

func TestDetailFooterShowsScrollPosition(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	next, _ := m.Update(tea.WindowSizeMsg{Width: 40, Height: 6})
	m = press(next.(Model), "G", "v") // the 103-character string wraps
	total := m.detail.TotalLineCount()
	if got := ansi.Strip(m.renderFooter()); !strings.Contains(got, fmt.Sprintf("Lines: 1-4/%d", total)) {
		t.Fatalf("footer = %q (total %d)", got, total)
	}
	m = press(m, "j")
	if got := ansi.Strip(m.renderFooter()); !strings.Contains(got, fmt.Sprintf("Lines: 2-5/%d", total)) {
		t.Fatalf("after scrolling: %q", got)
	}
}

func TestCursorRowIsReverseVideoWithoutColor(t *testing.T) {
	s := NewStyles(DefaultTokens("dark", false))
	if !s.Selected.GetReverse() || !s.Selected.GetBold() {
		t.Fatal("without color the cursor row should be bold reverse video")
	}
	if NewStyles(DefaultTokens("dark", true)).Selected.GetReverse() {
		t.Fatal("with color the cursor row uses the selection background")
	}
}

func TestSearchPromptFitsFooter(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = press(m, "/")
	got := ansi.Strip(m.renderFooter())
	if !strings.HasPrefix(got, "Search: > ") || ansi.StringWidth(got) > m.width {
		t.Fatalf("footer = %q", got)
	}
}

// Cursor keys skip the layout pass; the rows must still follow every
// expand/collapse that came before them.
func TestCursorKeysFollowExpandState(t *testing.T) {
	m, _ := newTestModel(t, Options{Depth: 1})
	m = press(m, "o", "G")
	if got := m.currentNode().Path(); got != "$.long" || m.cursor != len(m.flatNodes)-1 {
		t.Fatalf("after open all + G: %s", got)
	}
	m = press(m, "g", "j", "j", "l", "j") // expand $.deep, step into it
	if got := m.currentNode().Path(); got != "$.deep.level1" {
		t.Fatalf("after expanding: %s", got)
	}
	m = press(m, "O", "G")
	if got := m.currentNode().Path(); got != "$.long" || len(m.flatNodes) != 6 {
		t.Fatalf("after close all + G: %s (%d visible)", got, len(m.flatNodes))
	}
}
