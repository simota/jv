package tui

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/simota/jv/internal/parser"
)

type Options struct {
	Depth        int
	Theme        string
	ColorEnabled bool
	ShowTypes    bool
}

type Model struct {
	tree       *parser.Node
	flatNodes  []*parser.Node
	cursor     int
	viewport   viewport.Model
	styles     Styles
	tokens     Tokens
	showTypes  bool
	lines      []string
	lineIndex  map[*parser.Node]int
	width      int
	height     int
	statusMsg  string
	searchMode bool
	helpMode   bool
	search     textinput.Model
}

func NewModel(root *parser.Node, opts Options) Model {
	tokens := DefaultTokens(opts.Theme, opts.ColorEnabled)
	styles := NewStyles(tokens)
	applyExpandDepth(root, opts.Depth)

	vp := viewport.New(0, 0)
	search := textinput.New()
	search.Placeholder = "search"
	search.CharLimit = 256
	search.Width = 30

	m := Model{
		tree:      root,
		styles:    styles,
		tokens:    tokens,
		showTypes: opts.ShowTypes,
		viewport:  vp,
		search:    search,
		statusMsg: "",
	}
	m.rebuild()
	return m
}

func (m *Model) rebuild() {
	m.flatNodes = flattenVisible(m.tree)
	if len(m.flatNodes) == 0 {
		m.flatNodes = []*parser.Node{m.tree}
	}
	if m.cursor >= len(m.flatNodes) {
		m.cursor = len(m.flatNodes) - 1
	}
	if m.cursor < 0 {
		m.cursor = 0
	}
	m.lines, m.lineIndex = m.buildLines()
	m.viewport.SetContent(strings.Join(m.lines, "\n"))
	m.ensureCursorVisible()
}

func (m *Model) ensureCursorVisible() {
	if m.viewport.Height <= 0 {
		return
	}
	line := m.cursor
	if node := m.currentNode(); node != nil {
		if idx, ok := m.lineIndex[node]; ok {
			line = idx
		}
	}
	if line < m.viewport.YOffset {
		m.viewport.YOffset = line
		return
	}
	if line >= m.viewport.YOffset+m.viewport.Height {
		m.viewport.YOffset = line - m.viewport.Height + 1
	}
}

func applyExpandDepth(node *parser.Node, depth int) {
	if depth <= 0 {
		node.Expanded = node.Parent == nil
	} else {
		node.Expanded = node.Depth < depth
	}
	for _, child := range node.Children {
		applyExpandDepth(child, depth)
	}
}

func flattenVisible(node *parser.Node) []*parser.Node {
	out := []*parser.Node{node}
	if !node.Expanded {
		return out
	}
	for _, child := range node.Children {
		out = append(out, flattenVisible(child)...)
	}
	return out
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

func (m *Model) currentNodeJSON() (string, error) {
	node := m.currentNode()
	value := nodeToValue(node)
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return "", err
	}
	return string(data), nil
}

func nodeToValue(node *parser.Node) any {
	switch node.Type {
	case parser.TypeObject:
		obj := map[string]any{}
		for _, child := range node.Children {
			obj[child.Key] = nodeToValue(child)
		}
		return obj
	case parser.TypeArray:
		arr := make([]any, 0, len(node.Children))
		for _, child := range node.Children {
			arr = append(arr, nodeToValue(child))
		}
		return arr
	case parser.TypeString:
		if v, ok := node.Value.(string); ok {
			return v
		}
		return fmt.Sprintf("%v", node.Value)
	case parser.TypeNumber:
		return json.Number(node.StringValue())
	case parser.TypeBoolean:
		if v, ok := node.Value.(bool); ok {
			return v
		}
		parsed, err := strconv.ParseBool(node.StringValue())
		if err == nil {
			return parsed
		}
		return node.StringValue()
	case parser.TypeNull:
		return nil
	default:
		return node.StringValue()
	}
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
	step := m.viewport.Height
	if step <= 0 {
		step = 10
	}
	currentLine := m.currentLineIndex()
	target := currentLine + direction*step
	if target < 0 {
		target = 0
	}
	maxLine := len(m.lines) - 1
	if maxLine < 0 {
		maxLine = 0
	}
	if target > maxLine {
		target = maxLine
	}

	var best *parser.Node
	if direction >= 0 {
		for _, node := range m.flatNodes {
			if idx, ok := m.lineIndex[node]; ok && idx >= target {
				best = node
				break
			}
		}
		if best == nil {
			best = m.flatNodes[len(m.flatNodes)-1]
		}
	} else {
		for i := len(m.flatNodes) - 1; i >= 0; i-- {
			node := m.flatNodes[i]
			if idx, ok := m.lineIndex[node]; ok && idx <= target {
				best = node
				break
			}
		}
		if best == nil {
			best = m.flatNodes[0]
		}
	}

	m.setCursorToNode(best)
}

func (m *Model) currentLineIndex() int {
	if node := m.currentNode(); node != nil {
		if idx, ok := m.lineIndex[node]; ok {
			return idx
		}
	}
	return m.cursor
}

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

func (m *Model) findNextMatch(term string) *parser.Node {
	if term == "" || len(m.flatNodes) == 0 {
		return nil
	}
	lower := strings.ToLower(term)
	start := m.cursor + 1
	if start >= len(m.flatNodes) {
		start = 0
	}
	for i := 0; i < len(m.flatNodes); i++ {
		idx := (start + i) % len(m.flatNodes)
		node := m.flatNodes[idx]
		if strings.Contains(strings.ToLower(nodeSearchText(node)), lower) {
			return node
		}
	}
	return nil
}

func nodeSearchText(node *parser.Node) string {
	parts := []string{node.Key, string(node.Type), node.Path(), node.StringValue()}
	return strings.Join(parts, " ")
}

func formatNodeLabel(node *parser.Node, styles Styles) string {
	if node.Parent == nil {
		return styles.Key.Render("root")
	}
	if node.Parent.Type == parser.TypeArray {
		return styles.Key.Render("[" + node.Key + "]")
	}
	return styles.Key.Render(strconv.Quote(node.Key))
}

func formatNodeValue(node *parser.Node, styles Styles) string {
	switch node.Type {
	case parser.TypeString:
		return styles.String.Render(node.StringValue())
	case parser.TypeNumber:
		return styles.Number.Render(node.StringValue())
	case parser.TypeBoolean:
		return styles.Boolean.Render(node.StringValue())
	case parser.TypeNull:
		return styles.Null.Render("null")
	default:
		return ""
	}
}

func isLastChild(node *parser.Node) bool {
	if node.Parent == nil {
		return true
	}
	children := node.Parent.Children
	return len(children) > 0 && children[len(children)-1] == node
}

func hasNextSibling(node *parser.Node) bool {
	if node.Parent == nil {
		return false
	}
	children := node.Parent.Children
	for i, child := range children {
		if child == node {
			return i < len(children)-1
		}
	}
	return false
}

func treePrefix(node *parser.Node) string {
	if node.Parent == nil {
		return ""
	}
	ancestors := []*parser.Node{}
	for p := node.Parent; p != nil; p = p.Parent {
		ancestors = append([]*parser.Node{p}, ancestors...)
	}
	prefix := ""
	if len(ancestors) > 1 {
		for _, anc := range ancestors[:len(ancestors)-1] {
			if hasNextSibling(anc) {
				prefix += "|  "
			} else {
				prefix += "   "
			}
		}
	}
	if isLastChild(node) {
		prefix += "`- "
	} else {
		prefix += "|- "
	}
	return prefix
}

func containerSummary(node *parser.Node, styles Styles) string {
	switch node.Type {
	case parser.TypeObject:
		return " " + styles.TypeHint.Render("{"+itoa(len(node.Children))+"}")
	case parser.TypeArray:
		return " " + styles.TypeHint.Render("["+itoa(len(node.Children))+"]")
	default:
		return ""
	}
}

func itoa(v int) string {
	if v == 0 {
		return "0"
	}
	buf := [32]byte{}
	i := len(buf)
	for v > 0 {
		i--
		buf[i] = byte('0' + v%10)
		v /= 10
	}
	return string(buf[i:])
}
