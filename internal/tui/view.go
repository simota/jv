package tui

import (
	"fmt"
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
	header := joinWithGap(left, right, m.width)
	return m.styles.Header.Render(header)
}

func (m Model) renderFooter() string {
	node := m.currentNode()
	lines := fmt.Sprintf("Lines: %d/%d", m.cursor+1, len(m.flatNodes))
	depth := fmt.Sprintf("Depth: %d", node.Depth)
	footer := "Path: " + node.Path() + "  " + lines + "  " + depth
	if !m.searchMode && m.statusMsg != "" {
		footer = footer + "  " + m.statusMsg
	}
	footer = displaySafe(footer)
	if m.searchMode {
		footer = footer + "  Search: " + m.search.View()
	}
	if m.width > 0 && !m.searchMode {
		footer = ansi.Truncate(footer, m.width, "…")
	}

	return m.styles.Footer.Render(footer)
}

func (m Model) renderHelp() string {
	lines := []string{
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
	content := strings.Join(lines, "\n")
	if m.width > 0 {
		content = lipgloss.NewStyle().Width(m.width).Render(content)
	}
	return m.styles.Help.Render(content)
}

// openDetail shows the full selected value in a scrollable, wrapped view.
func (m *Model) openDetail() {
	node := m.currentNode()
	var b strings.Builder
	b.WriteString("Path: " + node.Path() + "\n")
	b.WriteString("Type: " + string(node.Type))
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

func (m Model) renderRow(r row) string {
	node := r.node
	indent := strings.Repeat(m.indentUnit(), node.Depth)
	comma := ""
	if !isLast(node) {
		comma = ","
	}
	if r.closing {
		return indent + m.containerClose(node) + comma
	}

	hlKey, hlValue := m.highlight(node)
	prefix := ""
	if node.Parent != nil && node.Parent.Type == parser.TypeObject {
		keyStyle := m.styles.Key
		if hlKey {
			keyStyle = m.styles.Match
		}
		prefix = keyStyle.Render(displaySafe(parser.QuoteString(node.Key))) + ": "
	}

	var line string
	switch {
	case node.Type != parser.TypeObject && node.Type != parser.TypeArray:
		value := formatNodeValue(node, m.styles)
		if hlValue {
			value = m.styles.Match.Render(displaySafe(node.StringValue()))
		}
		line = indent + prefix + value + comma
	case node.Expanded && len(node.Children) > 0:
		open := m.containerOpen(node)
		if hlValue {
			open = m.styles.Match.Render(open)
		}
		line = indent + prefix + open
	default:
		open := m.containerOpen(node)
		if hlValue {
			open = m.styles.Match.Render(open)
		}
		body := open + m.containerClose(node)
		if len(node.Children) > 0 {
			body = open + m.styles.TypeHint.Render("… "+containerSummary(node)) + m.containerClose(node)
		}
		line = indent + prefix + body + comma
	}
	line = m.attachTypeHint(line, node)
	if node == m.currentNode() {
		line = m.styles.Selected.Render(line)
	}
	return line
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

func (m Model) attachTypeHint(line string, node *parser.Node) string {
	if !m.showTypes {
		return line
	}
	hint := m.styles.TypeHint.Render("(" + string(node.Type) + ")")
	return line + " " + hint
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
