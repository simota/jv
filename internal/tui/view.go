package tui

import (
	"fmt"
	"strconv"
	"strings"
	"unicode"

	"github.com/charmbracelet/lipgloss"
	"github.com/charmbracelet/x/ansi"
	"github.com/simota/jv/internal/parser"
)

func (m Model) View() string {
	header := m.renderHeader()
	footer := m.renderFooter()

	if m.helpMode {
		return header + "\n" + m.renderHelp() + "\n" + footer
	}
	if m.detailMode {
		return header + "\n" + m.detail.View() + "\n" + footer
	}
	return header + "\n" + m.renderBody() + "\n" + footer
}

// renderBody renders only the rows inside the window.
func (m Model) renderBody() string {
	height := m.bodyHeight
	if height <= 0 {
		height = len(m.rows)
	}
	out := make([]string, 0, height)
	for i := m.yOffset; i < len(m.rows) && len(out) < height; i++ {
		line := m.renderRow(m.rows[i])
		if m.width > 0 {
			line = ansi.Truncate(line, m.width, "")
		}
		out = append(out, line)
	}
	for len(out) < height {
		out = append(out, "")
	}
	return strings.Join(out, "\n")
}

func (m Model) renderHeader() string {
	left := "jv - JSON Viewer"
	right := "[/] Search  [?] Help  [q] Quit"
	if m.detailMode {
		left = "jv - Value: " + displaySafe(m.currentNode().Path())
		right = "[y] Copy  [Esc] Back"
	}
	return m.styles.Header.Render(fitHeader(left, right, m.width))
}

// fitHeader puts left and right on one line of width cells. When they do
// not fit, left is shortened first so the key hints stay visible; on very
// narrow screens only left is shown.
func fitHeader(left, right string, width int) string {
	if width <= 0 || lipgloss.Width(left)+1+lipgloss.Width(right) <= width {
		return joinWithGap(left, right, width)
	}
	if budget := width - lipgloss.Width(right) - 1; budget >= minHeaderTitle {
		return joinWithGap(ansi.Truncate(left, budget, "…"), right, width)
	}
	return ansi.Truncate(left, width, "…")
}

// minHeaderTitle is the fewest cells left for the title before the key
// hints are dropped from a narrow header.
const minHeaderTitle = 8

const searchPrompt = "Search: "

func (m Model) renderFooter() string {
	if m.searchMode {
		return m.styles.Footer.Render(searchPrompt) + m.search.View()
	}
	node := m.currentNode()
	lines := fmt.Sprintf("Lines: %d/%d", m.cursor+1, len(m.flatNodes))
	if m.detailMode {
		total := m.detail.TotalLineCount()
		bottom := min(m.detail.YOffset+m.detail.Height, total)
		lines = fmt.Sprintf("Lines: %d-%d/%d", min(m.detail.YOffset+1, total), bottom, total)
	}
	info := displaySafe("Path: " + node.Path() + "  " + lines + "  Depth: " + strconv.Itoa(int(node.Depth)))
	return m.styles.Footer.Render(fitFooter(info, displaySafe(m.statusMsg), m.width))
}

// fitFooter joins the position info and the status message on one line of
// width cells. The status is feedback for the last key press, so the info
// is shortened first to keep it readable.
func fitFooter(info, status string, width int) string {
	if status == "" {
		if width > 0 {
			return ansi.Truncate(info, width, "…")
		}
		return info
	}
	full := info + "  " + status
	if width <= 0 || lipgloss.Width(full) <= width {
		return full
	}
	if avail := width - lipgloss.Width(status) - 2; avail >= 2 {
		return ansi.Truncate(info, avail, "…") + "  " + status
	}
	return ansi.Truncate(status, width, "…")
}

// searchInputWidth is the width of the search field that fits the footer
// next to the prompt ("Search: > " and the cursor cell).
func searchInputWidth(width int) int {
	const defaultWidth = 30
	if width <= 0 {
		return defaultWidth
	}
	return max(1, width-lipgloss.Width(searchPrompt+"> ")-1)
}

var helpLines = []string{
	"Navigate  Up/Down k/j  PgUp/PgDn Ctrl+u/d  g/G top/bottom",
	"  h / l         Collapse (or go to parent) / Expand",
	"  Space         Toggle",
	"  Enter         Toggle container, or view a leaf value",
	"  e             Expand everything under the cursor",
	"  o / O         Open all / Close all",
	"  1-9           Expand to depth",
	"Search",
	"  /text         Search keys and values in the whole document",
	"  /$.a.b[0]     Jump to a path (supports [*] and [-1])",
	"  n / N         Next / previous match     Esc  Clear highlight",
	"Values",
	"  v             Show full value (embedded JSON is pretty printed)",
	"  y / Y         Copy value as JSON / copy path (usable with jv -p)",
	"  t             Toggle type hints",
	"",
	"Press any key to close help, q to quit.",
}

// helpMore replaces the last visible help line when the help does not fit.
const helpMore = "… (enlarge the window to see all keys)"

// renderHelp fills the body area with the key reference, wrapped to the
// width and cut to the height so the header and footer stay on screen.
func (m Model) renderHelp() string {
	content := strings.Join(helpLines, "\n")
	if m.width > 0 {
		content = lipgloss.NewStyle().Width(m.width).Render(content)
	}
	lines := strings.Split(content, "\n")
	if m.bodyHeight > 0 {
		if len(lines) > m.bodyHeight {
			lines = append(lines[:m.bodyHeight-1], helpMore)
		}
		for len(lines) < m.bodyHeight {
			lines = append(lines, "")
		}
	}
	for i, l := range lines {
		l = strings.TrimRight(l, " ")
		if m.width > 0 {
			l = ansi.Truncate(l, m.width, "")
		}
		lines[i] = m.styles.Help.Render(l)
	}
	return strings.Join(lines, "\n")
}

// openDetail shows the full selected value in a scrollable, wrapped view.
func (m *Model) openDetail() {
	node := m.currentNode()
	var b strings.Builder
	b.WriteString("Path: " + node.Path() + "\n")
	b.WriteString("Type: " + node.Type.String())
	switch node.Type {
	case parser.TypeString:
		raw := node.RawString()
		fmt.Fprintf(&b, "  (%d chars)\n\n", len([]rune(raw)))
		b.WriteString(raw)
		if embedded := embeddedJSON(raw); embedded != "" {
			b.WriteString("\n\nEmbedded JSON:\n")
			b.WriteString(embedded)
		}
	case parser.TypeObject, parser.TypeArray:
		b.WriteString("  (" + containerSummary(node) + ")\n\n")
		b.WriteString(parser.Marshal(node, "  "))
	default:
		b.WriteString("\n\n" + node.StringValue())
	}
	content := displaySafe(b.String())
	if m.width > 0 {
		content = lipgloss.NewStyle().Width(m.width).Render(content)
	}
	m.detail.SetContent(content)
	m.detail.GotoTop()
	m.detailMode = true
}

func (m Model) detailCopyText() string {
	node := m.currentNode()
	if node.Type == parser.TypeString {
		return node.RawString()
	}
	return parser.Marshal(node, "  ")
}

// embeddedJSON pretty prints s if it is a JSON object or array encoded in a
// string (common in logs and API payloads).
func embeddedJSON(s string) string {
	t := strings.TrimSpace(s)
	if len(t) < 2 || (t[0] != '{' && t[0] != '[') {
		return ""
	}
	docs, err := parser.ParseBytes([]byte(t))
	if err != nil || len(docs) != 1 {
		return ""
	}
	return parser.Marshal(docs[0], "  ")
}

func isLast(node *parser.Node) bool {
	if node.Parent == nil {
		return true
	}
	siblings := node.Parent.Children
	return siblings[len(siblings)-1] == node
}

// segment is a run of row text drawn in one style.
type segment struct {
	text  string
	style lipgloss.Style
}

func (m Model) renderRow(r row) string {
	node := r.node
	plain := lipgloss.NewStyle()
	segs := []segment{{strings.Repeat(m.indentUnit(), int(node.Depth)), plain}}
	add := func(text string, style lipgloss.Style) { segs = append(segs, segment{text, style}) }
	comma := ""
	if !isLast(node) {
		comma = ","
	}
	if r.closing {
		add(m.containerClose(node)+comma, plain)
		return m.paintRow(node, segs)
	}

	hlKey, hlValue := m.highlight(node)
	if node.Parent != nil && node.Parent.Type == parser.TypeObject {
		keyStyle := m.styles.Key
		if hlKey {
			keyStyle = m.styles.Match
		}
		add(displaySafe(parser.QuoteString(node.Key)), keyStyle)
		add(": ", plain)
	}

	if node.Type != parser.TypeObject && node.Type != parser.TypeArray {
		style := m.valueStyle(node)
		if hlValue {
			style = m.styles.Match
		}
		add(displaySafe(node.StringValue()), style)
		add(comma, plain)
	} else {
		openStyle := plain
		if hlValue {
			openStyle = m.styles.Match
		}
		add(m.containerOpen(node), openStyle)
		if !node.Expanded || len(node.Children) == 0 {
			if len(node.Children) > 0 {
				add("… "+containerSummary(node), m.styles.TypeHint)
			}
			add(m.containerClose(node)+comma, plain)
		}
	}
	if m.showTypes {
		add(" ", plain)
		add("("+node.Type.String()+")", m.styles.TypeHint)
	}
	return m.paintRow(node, segs)
}

// paintRow renders the segments of a row. On the cursor row every segment
// also takes the selection style (its own foreground wins), so the
// highlight spans the whole row instead of stopping at the first reset.
func (m Model) paintRow(node *parser.Node, segs []segment) string {
	selected := node == m.currentNode()
	var b strings.Builder
	for _, s := range segs {
		if s.text == "" {
			continue
		}
		style := s.style
		if selected {
			style = style.Inherit(m.styles.Selected)
		}
		b.WriteString(style.Render(s.text))
	}
	return b.String()
}

// highlight decides whether the key and/or the value of a matched node is
// highlighted: path matches highlight the key (or the value when the node has
// no key); text matches highlight whichever part contains the search term.
func (m Model) highlight(node *parser.Node) (key, value bool) {
	if !m.matchSet[node] {
		return false, false
	}
	hasKey := node.Parent != nil && node.Parent.Type == parser.TypeObject
	if m.matchIsPath {
		return hasKey, !hasKey
	}
	term := strings.ToLower(m.matchLabel)
	key = hasKey && strings.Contains(strings.ToLower(node.Key), term)
	value = len(node.Children) == 0 && strings.Contains(strings.ToLower(node.RawString()), term)
	return key, value
}

func (m Model) indentUnit() string {
	return strings.Repeat(" ", m.tokens.Spacing.Indent)
}

func (m Model) containerOpen(node *parser.Node) string {
	if node.Type == parser.TypeArray {
		return "["
	}
	return "{"
}

func (m Model) containerClose(node *parser.Node) string {
	if node.Type == parser.TypeArray {
		return "]"
	}
	return "}"
}

func joinWithGap(left, right string, width int) string {
	if width <= 0 {
		return left + "  " + right
	}
	gap := width - lipgloss.Width(left) - lipgloss.Width(right)
	if gap < 1 {
		gap = 1
	}
	return left + strings.Repeat(" ", gap) + right
}

// displaySafe makes document text safe to write to the terminal. Control
// characters other than newline and tab (C0, DEL and C1, e.g. ESC, BEL, CR
// or U+009B CSI) are shown as escapes such as `\u001b`, so a document cannot
// set the window title, write the clipboard (OSC 52) or redraw the screen.
// Quoted JSON strings stay valid: the escapes are the JSON ones.
func displaySafe(s string) string {
	if !hasControl(s) {
		return s
	}
	var b strings.Builder
	b.Grow(len(s) + 16)
	for _, r := range s {
		switch {
		case r == '\n' || r == '\t':
			b.WriteRune(r)
		case r == '\r':
			b.WriteString(`\r`)
		case unicode.IsControl(r):
			fmt.Fprintf(&b, `\u%04x`, r)
		default:
			b.WriteRune(r)
		}
	}
	return b.String()
}

func hasControl(s string) bool {
	for i := 0; i < len(s); i++ {
		c := s[i]
		if (c < 0x20 && c != '\n' && c != '\t') || c == 0x7f {
			return true
		}
		// C1 controls U+0080..U+009F are encoded as 0xC2 0x80..0x9F
		if c == 0xc2 && i+1 < len(s) && s[i+1] >= 0x80 && s[i+1] <= 0x9f {
			return true
		}
	}
	return false
}
