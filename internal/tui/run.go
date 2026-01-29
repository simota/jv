package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/simota/jv/internal/parser"
)

func Run(root *parser.Node, opts Options) error {
	model := NewModel(root, opts)
	program := tea.NewProgram(model, tea.WithAltScreen())
	_, err := program.Run()
	return err
}
