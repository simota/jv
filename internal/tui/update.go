package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/simota/jv/internal/parser"
)

func (m Model) Init() tea.Cmd {
	return nil
}

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	if size, ok := msg.(tea.WindowSizeMsg); ok {
		m.width = size.Width
		m.height = size.Height
		available := size.Height - 2 // header + footer
		if available < 1 {
			available = 1
		}
		m.bodyHeight = available
		m.detail.Width = size.Width
		m.detail.Height = available
		m.search.Width = searchInputWidth(size.Width)
		if m.detailMode {
			m.openDetail()
		}
		m.rebuild()
		return m, nil
	}

	if m.searchMode {
		return m.updateSearch(msg)
	}
	if m.detailMode {
		return m.updateDetail(msg)
	}

	key, ok := msg.(tea.KeyMsg)
	if !ok {
		return m, nil
	}
	if m.helpMode && key.String() != "ctrl+c" && key.String() != "q" {
		m.helpMode = false
		return m, nil
	}

	switch key.String() {
	case "ctrl+c", "q":
		return m, tea.Quit
	case "up", "k":
		m.moveCursor(-1)
	case "down", "j":
		m.moveCursor(1)
	case "pgup", "ctrl+u":
		m.movePage(-1)
	case "pgdown", "ctrl+d":
		m.movePage(1)
	case "left", "h":
		node := m.currentNode()
		if len(node.Children) > 0 && node.Expanded {
			node.Expanded = false
		} else if node.Parent != nil {
			m.setCursorToNode(node.Parent)
		}
	case "right", "l":
		node := m.currentNode()
		if len(node.Children) > 0 && !node.Expanded {
			node.Expanded = true
		}
	case " ":
		m.toggleCurrent()
	case "enter":
		if len(m.currentNode().Children) > 0 {
			m.toggleCurrent()
		} else {
			m.openDetail()
			return m, nil
		}
	case "v":
		m.openDetail()
		return m, nil
	case "e":
		expandSubtree(m.currentNode())
		m.statusMsg = "Expanded " + m.currentNode().Path()
	case "o":
		m.expandAll(m.tree, true)
	case "O":
		m.expandAll(m.tree, false)
	case "g", "home":
		m.cursor = 0
	case "G", "end":
		m.cursor = len(m.flatNodes) - 1
	case "y":
		m.copy(m.currentNodeJSON(), "value")
	case "Y":
		m.copy(m.currentNode().Path(), "path "+m.currentNode().Path())
	case "t":
		m.showTypes = !m.showTypes
		if m.showTypes {
			m.statusMsg = "Types: on"
		} else {
			m.statusMsg = "Types: off"
		}
	case "?":
		m.helpMode = true
		return m, nil
	case "/":
		m.searchMode = true
		m.search.Focus()
		m.search.SetValue("")
		return m, nil
	case "n":
		if len(m.matches) == 0 {
			m.statusMsg = "No active search (press /)"
		} else {
			m.jumpToMatch(m.firstMatchFromCursor(false))
		}
	case "N":
		if len(m.matches) == 0 {
			m.statusMsg = "No active search (press /)"
		} else {
			m.jumpToMatch(m.firstMatchFromCursor(true) - 1)
		}
	case "esc":
		if len(m.matches) > 0 {
			m.clearMatches()
			m.statusMsg = "Search cleared"
		}
	default:
		s := key.String()
		if len(s) == 1 && s[0] >= '1' && s[0] <= '9' {
			current := m.currentNode()
			applyExpandDepth(m.tree, int(s[0]-'0'))
			m.layoutTree()
			// keep the cursor on the same node, or its nearest visible ancestor
			for n := current; n != nil; n = n.Parent {
				if visible(n) {
					m.setCursorToNode(n)
					break
				}
			}
		}
	}
	if keepsLayout[key.String()] {
		m.ensureCursorVisible()
	} else {
		m.rebuild()
	}
	return m, nil
}

// keepsLayout lists the keys that skip the O(visible nodes) layout pass at
// the end of Update: they move, copy or toggle hints without expanding or
// collapsing anything, or (n, N, 1-9) lay out the tree themselves when they
// change it. The layout is always current when Update is entered.
var keepsLayout = map[string]bool{
	"up": true, "k": true, "down": true, "j": true,
	"pgup": true, "ctrl+u": true, "pgdown": true, "ctrl+d": true,
	"g": true, "home": true, "G": true, "end": true,
	"y": true, "Y": true, "t": true, "esc": true,
	// these lay out the tree themselves when they expand something
	"n": true, "N": true,
	"1": true, "2": true, "3": true, "4": true, "5": true, "6": true, "7": true, "8": true, "9": true,
}

func visible(n *parser.Node) bool {
	for p := n.Parent; p != nil; p = p.Parent {
		if !p.Expanded {
			return false
		}
	}
	return true
}

func (m *Model) toggleCurrent() {
	node := m.currentNode()
	if len(node.Children) > 0 {
		node.Expanded = !node.Expanded
	}
}

func (m *Model) copy(text, what string) {
	if err := m.copyFn(text); err != nil {
		m.statusMsg = "Copy failed: " + err.Error()
		return
	}
	m.statusMsg = "Copied " + what
}

func (m Model) updateSearch(msg tea.Msg) (tea.Model, tea.Cmd) {
	if key, ok := msg.(tea.KeyMsg); ok {
		switch key.String() {
		case "esc":
			m.searchMode = false
			m.search.Blur()
			return m, nil
		case "enter":
			term := strings.TrimSpace(m.search.Value())
			m.searchMode = false
			m.search.Blur()
			if term == "" {
				m.statusMsg = "Search: empty"
				return m, nil
			}
			found, err := m.findMatches(term)
			if err != nil {
				m.statusMsg = "Search: " + err.Error()
				return m, nil
			}
			if len(found) == 0 {
				m.clearMatches()
				m.statusMsg = "Search: not found: " + term
				m.ensureCursorVisible() // nothing was expanded
				return m, nil
			}
			m.setMatches(found, term, strings.HasPrefix(term, "$"))
			m.jumpToMatch(m.firstMatchFromCursor(true)) // lays out if it expands
			m.ensureCursorVisible()
			return m, nil
		}
	}
	var cmd tea.Cmd
	m.search, cmd = m.search.Update(msg)
	return m, cmd
}

func (m Model) updateDetail(msg tea.Msg) (tea.Model, tea.Cmd) {
	key, ok := msg.(tea.KeyMsg)
	if !ok {
		return m, nil
	}
	switch key.String() {
	case "ctrl+c":
		return m, tea.Quit
	case "esc", "q", "v", "enter":
		m.detailMode = false
		return m, nil
	case "y":
		m.copy(m.detailCopyText(), "value")
		return m, nil
	case "Y":
		m.copy(m.currentNode().Path(), "path "+m.currentNode().Path())
		return m, nil
	}
	var cmd tea.Cmd
	m.detail, cmd = m.detail.Update(msg)
	return m, cmd
}
