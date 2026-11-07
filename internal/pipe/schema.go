package pipe

import (
	"bytes"
	"strings"

	"github.com/simota/jv/internal/parser"
)

// SchemaFormatter prints the inferred structure of a document. Array
// elements are merged so that every key that appears in any element is
// listed; keys missing from some elements are marked optional with `?` and
// values with several observed types are shown as a union (`string | null`).
type SchemaFormatter struct {
	color Colorizer
}

func NewSchemaFormatter(colorEnabled bool) *SchemaFormatter {
	return &SchemaFormatter{color: Colorizer{Enabled: colorEnabled}}
}

func (f *SchemaFormatter) Format(root *parser.Node) string {
	return f.FormatMany([]*parser.Node{root})
}

func (f *SchemaFormatter) FormatTo(out *Writer, root *parser.Node) {
	out.str(f.Format(root))
	out.maybeFlush()
}

// FormatMany prints a single schema merged from several documents (e.g. all
// lines of a JSON Lines file, or all results of a wildcard path).
func (f *SchemaFormatter) FormatMany(nodes []*parser.Node) string {
	s := &Schema{}
	for _, n := range nodes {
		s.merge(n)
	}
	var buf bytes.Buffer
	f.write(&buf, s, 0)
	buf.WriteByte('\n')
	return buf.String()
}

// Schema is a structural summary of one or more JSON values.
type Schema struct {
	primitives map[parser.NodeType]bool
	object     *objectSchema
	array      *arraySchema
}

type objectSchema struct {
	count  int
	order  []string
	fields map[string]*fieldSchema
}

type fieldSchema struct {
	seen   int
	schema *Schema
}

type arraySchema struct {
	items *Schema // nil when only empty arrays were observed
}

func (s *Schema) merge(node *parser.Node) {
	switch node.Type {
	case parser.TypeObject:
		if s.object == nil {
			s.object = &objectSchema{fields: map[string]*fieldSchema{}}
		}
		s.object.count++
		for _, child := range node.Children {
			field, ok := s.object.fields[child.Key]
			if !ok {
				field = &fieldSchema{schema: &Schema{}}
				s.object.fields[child.Key] = field
				s.object.order = append(s.object.order, child.Key)
			}
			field.seen++
			field.schema.merge(child)
		}
	case parser.TypeArray:
		if s.array == nil {
			s.array = &arraySchema{}
		}
		for _, child := range node.Children {
			if s.array.items == nil {
				s.array.items = &Schema{}
			}
			s.array.items.merge(child)
		}
	default:
		if s.primitives == nil {
			s.primitives = map[parser.NodeType]bool{}
		}
		s.primitives[node.Type] = true
	}
}

var primitiveOrder = []parser.NodeType{parser.TypeString, parser.TypeNumber, parser.TypeBoolean, parser.TypeNull}

func (s *Schema) isInline() bool {
	if s.object != nil && len(s.object.order) > 0 {
		return false
	}
	if s.array != nil && s.array.items != nil && !s.array.items.isInline() {
		return false
	}
	return true
}

func (f *SchemaFormatter) write(buf *bytes.Buffer, s *Schema, depth int) {
	parts := 0
	sep := func() {
		if parts > 0 {
			buf.WriteString(" | ")
		}
		parts++
	}
	if s.object != nil {
		sep()
		f.writeObject(buf, s.object, depth)
	}
	if s.array != nil {
		sep()
		f.writeArray(buf, s.array, depth)
	}
	for _, t := range primitiveOrder {
		if s.primitives[t] {
			sep()
			buf.WriteString(f.color.TypeHint(t.String()))
		}
	}
}

func (f *SchemaFormatter) writeObject(buf *bytes.Buffer, o *objectSchema, depth int) {
	if len(o.order) == 0 {
		buf.WriteString("{}")
		return
	}
	buf.WriteString("{\n")
	indent := strings.Repeat("  ", depth+1)
	for i, key := range o.order {
		field := o.fields[key]
		buf.WriteString(indent)
		buf.WriteString(f.color.Key(parser.QuoteString(key)))
		if field.seen < o.count {
			buf.WriteString(f.color.TypeHint("?"))
		}
		buf.WriteString(": ")
		f.write(buf, field.schema, depth+1)
		if i < len(o.order)-1 {
			buf.WriteString(",")
		}
		buf.WriteByte('\n')
	}
	buf.WriteString(strings.Repeat("  ", depth))
	buf.WriteString("}")
}

func (f *SchemaFormatter) writeArray(buf *bytes.Buffer, a *arraySchema, depth int) {
	if a.items == nil {
		buf.WriteString("[]")
		return
	}
	if a.items.isInline() {
		buf.WriteString("[")
		f.write(buf, a.items, depth)
		buf.WriteString("]")
		return
	}
	buf.WriteString("[\n")
	buf.WriteString(strings.Repeat("  ", depth+1))
	f.write(buf, a.items, depth+1)
	buf.WriteByte('\n')
	buf.WriteString(strings.Repeat("  ", depth))
	buf.WriteString("]")
}
