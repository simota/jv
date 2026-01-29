package tui

import (
	"strings"

	"github.com/atotto/clipboard"
	tea "github.com/charmbracelet/bubbletea"
)

func (m Model) Init() tea.Cmd {
	return nil
}

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch typed := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = typed.Width
		m.height = typed.Height
		headHeight := 1
		footerHeight := 1
		available := typed.Height - headHeight - footerHeight
		if available < 1 {
			available = 1
		}
		m.viewport.Width = typed.Width
		m.viewport.Height = available
		m.rebuild()
		return m, nil
	}

	if m.searchMode {
		if key, ok := msg.(tea.KeyMsg); ok {
			switch key.String() {
			case "esc":
				m.searchMode = false
				m.search.Blur()
				return m, nil
			case "enter":
				term := strings.TrimSpace(m.search.Value())
				m.search.SetValue("")
				m.searchMode = false
				m.search.Blur()
				if term == "" {
					m.statusMsg = "Search: empty"
					return m, nil
				}
				if match := m.findNextMatch(term); match != nil {
					m.setCursorToNode(match)
					m.statusMsg = "Search: " + term
					m.rebuild()
					return m, nil
				}
				m.statusMsg = "Search: not found"
				return m, nil
			}
		}
		var cmd tea.Cmd
		m.search, cmd = m.search.Update(msg)
		return m, cmd
	}

	if key, ok := msg.(tea.KeyMsg); ok {
		switch key.String() {
		case "ctrl+c", "q":
			return m, tea.Quit
		case "up", "k":
			m.moveCursor(-1)
			m.rebuild()
		case "down", "j":
			m.moveCursor(1)
			m.rebuild()
		case "pgup", "ctrl+u":
			m.movePage(-1)
			m.rebuild()
		case "pgdown", "ctrl+d":
			m.movePage(1)
			m.rebuild()
		case "left", "h":
			node := m.currentNode()
			if len(node.Children) > 0 && node.Expanded {
				node.Expanded = false
				m.rebuild()
				return m, nil
			}
			if node.Parent != nil {
				m.setCursorToNode(node.Parent)
				m.rebuild()
			}
		case "right", "l":
			node := m.currentNode()
			if len(node.Children) > 0 && !node.Expanded {
				node.Expanded = true
				m.rebuild()
			}
		case " ", "enter":
			node := m.currentNode()
			if len(node.Children) > 0 {
				node.Expanded = !node.Expanded
				m.rebuild()
			}
		case "o":
			m.expandAll(m.tree, true)
			m.rebuild()
		case "O":
			m.expandAll(m.tree, false)
			m.rebuild()
		case "g":
			m.cursor = 0
			m.rebuild()
		case "G":
			m.cursor = len(m.flatNodes) - 1
			m.rebuild()
		case "y":
			value, err := m.currentNodeJSON()
			if err != nil {
				m.statusMsg = "Copy failed"
				return m, nil
			}
			if err := clipboard.WriteAll(value); err != nil {
				m.statusMsg = "Copy failed"
				return m, nil
			}
			m.statusMsg = "Copied value"
		case "t":
			m.showTypes = !m.showTypes
			if m.showTypes {
				m.statusMsg = "Types: on"
			} else {
				m.statusMsg = "Types: off"
			}
			m.rebuild()
		case "?":
			m.helpMode = !m.helpMode
		case "/":
			m.searchMode = true
			m.search.Focus()
			m.search.SetValue("")
			return m, nil
		default:
			if len(key.String()) == 1 {
				r := key.String()[0]
				if r >= '1' && r <= '9' {
					depth := int(r - '0')
					applyExpandDepth(m.tree, depth)
					m.rebuild()
				}
			}
		}
	}

	return m, nil
}
