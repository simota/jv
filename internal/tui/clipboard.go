package tui

import (
	"os"

	"github.com/atotto/clipboard"
	osc52 "github.com/aymanbagabas/go-osc52/v2"
)

// copyToClipboard tries the system clipboard first and falls back to the
// OSC 52 terminal escape sequence, which also works over SSH and inside
// tmux/screen when the local clipboard tools are unavailable.
func copyToClipboard(s string) error {
	if err := clipboard.WriteAll(s); err == nil {
		return nil
	}
	seq := osc52.New(s)
	if os.Getenv("TMUX") != "" {
		seq = seq.Tmux()
	} else if os.Getenv("STY") != "" {
		seq = seq.Screen()
	}
	_, err := seq.WriteTo(os.Stderr)
	return err
}
