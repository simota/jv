package pipe

import (
	"strings"

	"github.com/simota/jv/internal/parser"
)

type TypedFormatter struct {
	color Colorizer
}

func NewTypedFormatter(colorEnabled bool) *TypedFormatter {
	return &TypedFormatter{color: Colorizer{Enabled: colorEnabled}}
}

func (f *TypedFormatter) Format(root *parser.Node) string {
	lines := make([]string, 0)
	f.walk(root, "", true, &lines)
	return strings.Join(lines, "\n") + "\n"
}

func (f *TypedFormatter) walk(node *parser.Node, prefix string, isLast bool, lines *[]string) {
	linePrefix := prefix
	if node.Parent != nil {
		if isLast {
			linePrefix += "`- "
		} else {
			linePrefix += "|- "
		}
	}

	label := f.nodeLabel(node)
	value := f.nodeValue(node)
	if value != "" {
		label = label + ": " + value
	}
	typeHint := f.color.TypeHint(string(node.Type))
	line := label
	if typeHint != "" {
		line = line + "  " + typeHint
	}
	*lines = append(*lines, linePrefix+line)

	if len(node.Children) == 0 {
		return
	}

	nextPrefix := prefix
	if node.Parent != nil {
		if isLast {
			nextPrefix += "   "
		} else {
			nextPrefix += "|  "
		}
	}
	for i, child := range node.Children {
		f.walk(child, nextPrefix, i == len(node.Children)-1, lines)
	}
}

func (f *TypedFormatter) nodeLabel(node *parser.Node) string {
	if node.Parent == nil {
		return f.color.Key("root") + f.containerHint(node)
	}
	if node.Parent.Type == parser.TypeArray {
		return f.color.Key("["+node.Key+"]") + f.containerHint(node)
	}
	return f.color.Key("\""+node.Key+"\"") + f.containerHint(node)
}

func (f *TypedFormatter) containerHint(node *parser.Node) string {
	switch node.Type {
	case parser.TypeObject:
		return " " + f.color.TypeHint("{"+itoa(len(node.Children))+"}")
	case parser.TypeArray:
		return " " + f.color.TypeHint("["+itoa(len(node.Children))+"]")
	default:
		return ""
	}
}

func (f *TypedFormatter) nodeValue(node *parser.Node) string {
	switch node.Type {
	case parser.TypeString:
		return f.color.String(node.StringValue())
	case parser.TypeNumber:
		return f.color.Number(node.StringValue())
	case parser.TypeBoolean:
		return f.color.Boolean(node.StringValue())
	case parser.TypeNull:
		return f.color.Null("null")
	default:
		return ""
	}
}
