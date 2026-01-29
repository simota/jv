package tui

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/simota/jv/internal/parser"
)

func (m Model) View() string {
	header := m.renderHeader()
	footer := m.renderFooter()

	if m.helpMode {
		help := m.renderHelp()
		return header + "\n" + help + "\n" + footer
	}

	body := m.viewport.View()
	return header + "\n" + body + "\n" + footer
}

func (m Model) renderHeader() string {
	left := "jv - JSON Viewer"
	right := "[?] Help  [q] Quit"
	header := joinWithGap(left, right, m.width)
	return m.styles.Header.Render(header)
}

func (m Model) renderFooter() string {
	node := m.currentNode()
	path := node.Path()
	lines := fmt.Sprintf("Lines: %d/%d", m.cursor+1, len(m.flatNodes))
	depth := fmt.Sprintf("Depth: %d", node.Depth)
	left := "Path: " + path
	mid := lines + "  " + depth
	footer := left + "  " + mid

	if m.searchMode {
		footer = footer + "  Search: " + m.search.View()
	} else if m.statusMsg != "" {
		footer = footer + "  " + m.statusMsg
	}

	return m.styles.Footer.Render(footer)
}

func (m Model) renderHelp() string {
	lines := []string{
		"Keys:",
		"  Up/Down or k/j : Move",
		"  PgUp/PgDn or Ctrl+u/Ctrl+d : Page",
		"  Left/Right or h/l : Collapse/Expand",
		"  Enter/Space : Toggle",
		"  o / O : Open all / Close all",
		"  1-9 : Expand to depth",
		"  g / G : Top / Bottom",
		"  / : Search",
		"  t : Toggle type hints",
		"  y : Copy value",
		"  ? : Toggle help",
		"  q : Quit",
	}
	content := strings.Join(lines, "\n")
	if m.width > 0 {
		content = lipgloss.NewStyle().Width(m.width).Render(content)
	}
	return m.styles.Help.Render(content)
}

func (m Model) buildLines() ([]string, map[*parser.Node]int) {
	lines := []string{}
	lineIndex := map[*parser.Node]int{}
	m.renderJSON(&lines, lineIndex, m.tree, 0, true)
	if len(lines) == 0 {
		return []string{""}, lineIndex
	}
	return lines, lineIndex
}

func (m Model) renderJSON(lines *[]string, lineIndex map[*parser.Node]int, node *parser.Node, depth int, isLast bool) {
	indent := strings.Repeat(m.indentUnit(), depth)
	comma := ""
	if !isLast {
		comma = ","
	}
	if node.Parent == nil {
		m.renderRoot(lines, lineIndex, node, indent)
		return
	}
	prefix := ""
	if node.Parent.Type == parser.TypeObject {
		prefix = strconv.Quote(node.Key) + ": "
	}
	if node.Type == parser.TypeObject || node.Type == parser.TypeArray {
		if node.Expanded {
			open := indent + prefix + m.containerOpen(node)
			m.addLine(lines, lineIndex, node, m.attachTypeHint(open, node))
			for i, child := range node.Children {
				m.renderJSON(lines, lineIndex, child, depth+1, i == len(node.Children)-1)
			}
			close := indent + m.containerClose(node) + comma
			m.addLine(lines, lineIndex, nil, close)
			return
		}
		open := indent + prefix + m.containerOpen(node)
		m.addLine(lines, lineIndex, node, m.attachTypeHint(open, node))
		placeholder := indent + m.indentUnit() + "..."
		m.addLine(lines, lineIndex, nil, placeholder)
		close := indent + m.containerClose(node) + comma
		m.addLine(lines, lineIndex, nil, close)
		return
	}
	value := formatNodeValue(node, m.styles)
	line := indent + prefix + value
	m.addLine(lines, lineIndex, node, m.attachTypeHint(line, node)+comma)
}

func (m Model) renderRoot(lines *[]string, lineIndex map[*parser.Node]int, node *parser.Node, indent string) {
	if node.Type == parser.TypeObject || node.Type == parser.TypeArray {
		if node.Expanded {
			open := indent + m.containerOpen(node)
			m.addLine(lines, lineIndex, node, m.attachTypeHint(open, node))
			for i, child := range node.Children {
				m.renderJSON(lines, lineIndex, child, 1, i == len(node.Children)-1)
			}
			close := indent + m.containerClose(node)
			m.addLine(lines, lineIndex, nil, close)
			return
		}
		open := indent + m.containerOpen(node)
		m.addLine(lines, lineIndex, node, m.attachTypeHint(open, node))
		placeholder := indent + m.indentUnit() + "..."
		m.addLine(lines, lineIndex, nil, placeholder)
		close := indent + m.containerClose(node)
		m.addLine(lines, lineIndex, nil, close)
		return
	}
	value := formatNodeValue(node, m.styles)
	m.addLine(lines, lineIndex, node, m.attachTypeHint(indent+value, node))
}

func (m Model) addLine(lines *[]string, lineIndex map[*parser.Node]int, node *parser.Node, line string) {
	if node != nil && node == m.currentNode() {
		line = m.styles.Selected.Render(line)
	}
	idx := len(*lines)
	*lines = append(*lines, line)
	if node != nil {
		lineIndex[node] = idx
	}
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

func (m Model) containerCollapsed(node *parser.Node) string {
	if node.Type == parser.TypeArray {
		return "[...]"
	}
	return "{...}"
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
