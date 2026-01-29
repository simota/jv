package tui

import "github.com/charmbracelet/lipgloss"

type Styles struct {
	Key      lipgloss.Style
	String   lipgloss.Style
	Number   lipgloss.Style
	Boolean  lipgloss.Style
	Null     lipgloss.Style
	TypeHint lipgloss.Style
	Selected lipgloss.Style
	Header   lipgloss.Style
	Footer   lipgloss.Style
	Help     lipgloss.Style
}

func NewStyles(tokens Tokens) Styles {
	base := lipgloss.NewStyle()
	styles := Styles{
		Key:      base,
		String:   base,
		Number:   base,
		Boolean:  base,
		Null:     base,
		TypeHint: base,
		Selected: base.Bold(tokens.Typography.SelectedBold),
		Header:   base.Bold(tokens.Typography.HeaderBold),
		Footer:   base,
		Help:     base,
	}

	if tokens.Colors.Key == "" {
		return styles
	}

	styles.Key = base.Foreground(lipgloss.Color(tokens.Colors.Key))
	styles.String = base.Foreground(lipgloss.Color(tokens.Colors.String))
	styles.Number = base.Foreground(lipgloss.Color(tokens.Colors.Number))
	styles.Boolean = base.Foreground(lipgloss.Color(tokens.Colors.Boolean))
	styles.Null = base.Foreground(lipgloss.Color(tokens.Colors.Null))
	styles.TypeHint = base.Foreground(lipgloss.Color(tokens.Colors.TypeHint))
	styles.Selected = base.Background(lipgloss.Color(tokens.Colors.SelectedBg)).Foreground(lipgloss.Color(tokens.Colors.SelectedFg))
	styles.Header = base.Bold(tokens.Typography.HeaderBold).Foreground(lipgloss.Color(tokens.Colors.Header))
	styles.Footer = base.Foreground(lipgloss.Color(tokens.Colors.Footer))
	styles.Help = base.Foreground(lipgloss.Color(tokens.Colors.Help))

	return styles
}
