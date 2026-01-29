package parser

import (
	"encoding/json"
	"fmt"
	"io"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

type NodeType string

const (
	TypeObject  NodeType = "object"
	TypeArray   NodeType = "array"
	TypeString  NodeType = "string"
	TypeNumber  NodeType = "number"
	TypeBoolean NodeType = "boolean"
	TypeNull    NodeType = "null"
)

type Node struct {
	Key      string
	Value    any
	Type     NodeType
	Children []*Node
	Parent   *Node
	Depth    int
	Expanded bool
}

func Parse(r io.Reader) (*Node, error) {
	dec := json.NewDecoder(r)
	dec.UseNumber()
	var v any
	if err := dec.Decode(&v); err != nil {
		return nil, err
	}
	root := buildNode("root", v, nil, 0)
	return root, nil
}

func buildNode(key string, v any, parent *Node, depth int) *Node {
	node := &Node{
		Key:    key,
		Value:  v,
		Parent: parent,
		Depth:  depth,
	}

	switch val := v.(type) {
	case map[string]any:
		node.Type = TypeObject
		keys := make([]string, 0, len(val))
		for k := range val {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			child := buildNode(k, val[k], node, depth+1)
			node.Children = append(node.Children, child)
		}
		node.Value = nil
	case []any:
		node.Type = TypeArray
		for i, item := range val {
			child := buildNode(strconv.Itoa(i), item, node, depth+1)
			node.Children = append(node.Children, child)
		}
		node.Value = nil
	case json.Number:
		node.Type = TypeNumber
		node.Value = val.String()
	case string:
		node.Type = TypeString
		node.Value = val
	case bool:
		node.Type = TypeBoolean
		node.Value = val
	case nil:
		node.Type = TypeNull
		node.Value = nil
	case float64:
		node.Type = TypeNumber
		node.Value = strconv.FormatFloat(val, 'f', -1, 64)
	case int:
		node.Type = TypeNumber
		node.Value = strconv.Itoa(val)
	case int64:
		node.Type = TypeNumber
		node.Value = strconv.FormatInt(val, 10)
	default:
		node.Type = TypeString
		node.Value = fmt.Sprintf("%v", val)
	}

	return node
}

func (n *Node) Path() string {
	if n.Parent == nil {
		return "$"
	}
	parentPath := n.Parent.Path()
	if n.Parent.Type == TypeArray {
		return parentPath + "[" + n.Key + "]"
	}
	if isSimpleKey(n.Key) {
		return parentPath + "." + n.Key
	}
	return parentPath + "[" + strconv.Quote(n.Key) + "]"
}

var simpleKeyRe = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)

func isSimpleKey(key string) bool {
	return simpleKeyRe.MatchString(key)
}

func (n *Node) StringValue() string {
	switch n.Type {
	case TypeString:
		if v, ok := n.Value.(string); ok {
			return strconv.Quote(v)
		}
		return strconv.Quote(fmt.Sprintf("%v", n.Value))
	case TypeNumber:
		return fmt.Sprintf("%v", n.Value)
	case TypeBoolean:
		if v, ok := n.Value.(bool); ok {
			return strconv.FormatBool(v)
		}
		return fmt.Sprintf("%v", n.Value)
	case TypeNull:
		return "null"
	default:
		return strings.TrimSpace(fmt.Sprintf("%v", n.Value))
	}
}
