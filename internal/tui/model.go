package tui

import (
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/charmbracelet/bubbles/textinput"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/simota/jv/internal/parser"
)

type Options struct {
	Depth        int
	Theme        string
	ColorEnabled bool
	ShowTypes    bool
	// Focus lists nodes to jump to on start (e.g. results of --path). They
	// become the navigable match list for n/N.
	Focus      []*parser.Node
	FocusLabel string
}

type Model struct {
	tree *parser.Node
	// flatNodes are the visible nodes; rows are the visible lines (nodes
	// plus closing brackets). nodeRow[i] is the row of flatNodes[i]. Only the
	// rows inside the window are rendered, so large documents stay fast.
	flatNodes  []*parser.Node
	nodeRow    []int
	rows       []row
	cursor     int
	yOffset    int
	bodyHeight int
	styles     Styles
	tokens     Tokens
	showTypes  bool
	width      int
	height     int
	statusMsg  string
	searchMode bool
	helpMode   bool
	search     textinput.Model

	// search / path results
	matches     []*parser.Node
	matchSet    map[*parser.Node]bool
	matchIdx    int
	matchLabel  string
	matchIsPath bool

	// detail view of the selected value
	detailMode bool
	detail     viewport.Model

	// clipboard is swappable for tests
	copyFn func(string) error
}

func NewModel(root *parser.Node, opts Options) Model {
	tokens := DefaultTokens(opts.Theme, opts.ColorEnabled)
	styles := NewStyles(tokens)
	applyExpandDepth(root, opts.Depth)

	search := textinput.New()
	search.Placeholder = "text or $.path"
	search.CharLimit = 256
	search.Width = searchInputWidth(0)

	m := Model{
		tree:      root,
		styles:    styles,
		tokens:    tokens,
		showTypes: opts.ShowTypes,
		detail:    viewport.New(0, 0),
		search:    search,
		copyFn:    copyToClipboard,
	}
	if len(opts.Focus) > 0 {
		m.setMatches(opts.Focus, opts.FocusLabel, true)
		m.jumpToMatch(0)
	}
	m.rebuild()
	return m
}

// row is one rendered line: a node, or the closing bracket of a container.
type row struct {
	node    *parser.Node
	closing bool
}

func (m *Model) rebuild() {
	m.flatNodes, m.nodeRow, m.rows = layout(m.tree)
	if m.cursor >= len(m.flatNodes) {
		m.cursor = len(m.flatNodes) - 1
	}
	if m.cursor < 0 {
		m.cursor = 0
	}
	m.ensureCursorVisible()
}

func layout(root *parser.Node) ([]*parser.Node, []int, []row) {
	// Count first so that the slices are allocated once at their final size
	// (growing them by appending copies them repeatedly on large trees).
	numNodes, numRows := countVisible(root)
	nodes := make([]*parser.Node, 0, numNodes)
	nodeRow := make([]int, 0, numNodes)
	rows := make([]row, 0, numRows)
	var walk func(n *parser.Node)
	walk = func(n *parser.Node) {
		nodes = append(nodes, n)
		nodeRow = append(nodeRow, len(rows))
		rows = append(rows, row{node: n})
		if n.Expanded && len(n.Children) > 0 {
			for _, child := range n.Children {
				walk(child)
			}
			rows = append(rows, row{node: n, closing: true})
		}
	}
	walk(root)
	return nodes, nodeRow, rows
}

// countVisible returns the number of visible nodes and rows below root.
func countVisible(root *parser.Node) (nodes, rows int) {
	nodes, rows = 1, 1
	if root.Expanded && len(root.Children) > 0 {
		for _, child := range root.Children {
			n, r := countVisible(child)
			nodes += n
			rows += r
		}
		rows++ // closing bracket
	}
	return nodes, rows
}

func (m *Model) ensureCursorVisible() {
	if m.bodyHeight <= 0 {
		return
	}
	line := m.currentLineIndex()
	if line < m.yOffset {
		m.yOffset = line
	}
	if line >= m.yOffset+m.bodyHeight {
		m.yOffset = line - m.bodyHeight + 1
	}
	if maxOffset := len(m.rows) - m.bodyHeight; m.yOffset > maxOffset {
		m.yOffset = maxOffset
	}
	if m.yOffset < 0 {
		m.yOffset = 0
	}
}

func applyExpandDepth(node *parser.Node, depth int) {
	if depth <= 0 {
		node.Expanded = node.Parent == nil
	} else {
		node.Expanded = int(node.Depth) < depth
	}
	for _, child := range node.Children {
		applyExpandDepth(child, depth)
	}
}

func flattenVisible(node *parser.Node) []*parser.Node {
	nodes, _, _ := layout(node)
	return nodes
}

func (m *Model) currentNode() *parser.Node {
	if len(m.flatNodes) == 0 {
		return m.tree
	}
	if m.cursor < 0 || m.cursor >= len(m.flatNodes) {
		return m.tree
	}
	return m.flatNodes[m.cursor]
}

func (m *Model) currentNodeJSON() string {
	return parser.Marshal(m.currentNode(), "  ")
}

func (m *Model) moveCursor(delta int) {
	if len(m.flatNodes) == 0 {
		return
	}
	m.cursor += delta
	if m.cursor < 0 {
		m.cursor = 0
	}
	if m.cursor >= len(m.flatNodes) {
		m.cursor = len(m.flatNodes) - 1
	}
}

func (m *Model) movePage(direction int) {
	if len(m.flatNodes) == 0 {
		return
	}
	step := m.bodyHeight
	if step <= 0 {
		step = 10
	}
	target := m.currentLineIndex() + direction*step
	// nodeRow is increasing: find the first node at/after (down) or the last
	// node at/before (up) the target row.
	idx := sort.SearchInts(m.nodeRow, target)
	if direction < 0 && (idx >= len(m.nodeRow) || m.nodeRow[idx] > target) {
		idx--
	}
	if idx < 0 {
		idx = 0
	}
	if idx >= len(m.flatNodes) {
		idx = len(m.flatNodes) - 1
	}
	m.cursor = idx
}

func (m *Model) currentLineIndex() int {
	if m.cursor >= 0 && m.cursor < len(m.nodeRow) {
		return m.nodeRow[m.cursor]
	}
	return m.cursor
}

// setCursorToNode moves the cursor to target if it is visible.
func (m *Model) setCursorToNode(target *parser.Node) {
	if target == nil {
		return
	}
	for i, node := range m.flatNodes {
		if node == target {
			m.cursor = i
			return
		}
	}
}

// revealNode expands every ancestor of target, rebuilds the visible list and
// moves the cursor to it.
func (m *Model) revealNode(target *parser.Node) {
	for p := target.Parent; p != nil; p = p.Parent {
		p.Expanded = true
	}
	m.flatNodes = flattenVisible(m.tree)
	m.setCursorToNode(target)
}

func (m *Model) expandAll(node *parser.Node, expanded bool) {
	if node == nil {
		return
	}
	if node.Parent == nil {
		node.Expanded = true
	} else {
		node.Expanded = expanded
	}
	for _, child := range node.Children {
		m.expandAll(child, expanded)
	}
}

func expandSubtree(node *parser.Node) {
	parser.Walk(node, func(n *parser.Node) bool {
		if len(n.Children) > 0 {
			n.Expanded = true
		}
		return true
	})
}

// findMatches searches keys and values of the whole tree (including
// collapsed parts). Terms starting with `$` are evaluated as paths.
func (m *Model) findMatches(term string) ([]*parser.Node, error) {
	if strings.HasPrefix(term, "$") {
		q, err := parser.CompilePath(term)
		if err != nil {
			return nil, err
		}
		return q.Find(m.tree), nil
	}
	lower := strings.ToLower(term)
	out := []*parser.Node{}
	parser.Walk(m.tree, func(n *parser.Node) bool {
		if n.Parent != nil && n.Parent.Type == parser.TypeObject && containsFold(n.Key, lower) {
			out = append(out, n)
			return true
		}
		if len(n.Children) == 0 && n.Type != parser.TypeObject && n.Type != parser.TypeArray &&
			containsFold(n.RawString(), lower) {
			out = append(out, n)
		}
		return true
	})
	return out, nil
}

// containsFold reports strings.Contains(strings.ToLower(s), lower), where
// lower is already lower case. ASCII text (the common case) is compared
// without building the lower-cased copy.
func containsFold(s, lower string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= utf8.RuneSelf {
			return strings.Contains(strings.ToLower(s), lower)
		}
	}
	// s is ASCII, so its lower-cased form is ASCII too and can only contain
	// an ASCII term.
	for i := 0; i < len(lower); i++ {
		if lower[i] >= utf8.RuneSelf {
			return false
		}
	}
	for i := 0; i+len(lower) <= len(s); i++ {
		j := 0
		for j < len(lower) && toLowerASCII(s[i+j]) == lower[j] {
			j++
		}
		if j == len(lower) {
			return true
		}
	}
	return false
}

func toLowerASCII(c byte) byte {
	if 'A' <= c && c <= 'Z' {
		return c + ('a' - 'A')
	}
	return c
}

func (m *Model) setMatches(nodes []*parser.Node, label string, isPath bool) {
	m.matches = nodes
	m.matchIsPath = isPath
	m.matchLabel = label
	m.matchIdx = 0
	m.matchSet = make(map[*parser.Node]bool, len(nodes))
	for _, n := range nodes {
		m.matchSet[n] = true
	}
}

func (m *Model) clearMatches() {
	m.matches = nil
	m.matchSet = nil
	m.matchLabel = ""
	m.matchIdx = 0
}

// firstMatchFromCursor returns the index of the first match at or after the
// current node in document order (wrapping around).
func (m *Model) firstMatchFromCursor(includeCurrent bool) int {
	cur := m.currentNode()
	passed := false
	found := -1
	idx := 0 // index into m.matches (which are in document order)
	parser.Walk(m.tree, func(n *parser.Node) bool {
		if found >= 0 || idx >= len(m.matches) {
			return false
		}
		if n == cur {
			passed = true
			if includeCurrent && m.matches[idx] == n {
				found = idx
				return false
			}
		} else if passed && m.matches[idx] == n {
			found = idx
			return false
		}
		if m.matches[idx] == n {
			idx++
		}
		return true
	})
	if found < 0 {
		return 0
	}
	return found
}

func (m *Model) jumpToMatch(i int) {
	if len(m.matches) == 0 {
		return
	}
	n := len(m.matches)
	m.matchIdx = ((i % n) + n) % n
	m.revealNode(m.matches[m.matchIdx])
	m.statusMsg = m.matchStatus()
}

func (m *Model) matchStatus() string {
	if len(m.matches) == 0 {
		return ""
	}
	return "Match " + strconv.Itoa(m.matchIdx+1) + "/" + strconv.Itoa(len(m.matches)) + ": " + m.matchLabel + "  (n/N)"
}

// valueStyle is the syntax color of a scalar value.
func (m Model) valueStyle(node *parser.Node) lipgloss.Style {
	switch node.Type {
	case parser.TypeString:
		return m.styles.String
	case parser.TypeNumber:
		return m.styles.Number
	case parser.TypeBoolean:
		return m.styles.Boolean
	default:
		return m.styles.Null
	}
}

func containerSummary(node *parser.Node) string {
	n := len(node.Children)
	switch node.Type {
	case parser.TypeObject:
		if n == 1 {
			return "1 key"
		}
		return strconv.Itoa(n) + " keys"
	case parser.TypeArray:
		if n == 1 {
			return "1 item"
		}
		return strconv.Itoa(n) + " items"
	default:
		return ""
	}
}
