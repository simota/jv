package pipe

import (
	"io"

	"github.com/simota/jv/internal/parser"
)

// flushSize is the amount of buffered output that triggers a write.
const flushSize = 64 << 10

// Writer buffers formatted output. Formatters append to it directly, so no
// intermediate strings are built per value, and output is passed on to the
// underlying io.Writer in large chunks instead of being held in memory as a
// whole. The first write error is kept and reported by Flush; later output
// is discarded.
type Writer struct {
	buf []byte
	w   io.Writer
	err error
}

// NewWriter returns a Writer that writes to w.
func NewWriter(w io.Writer) *Writer {
	return &Writer{buf: make([]byte, 0, flushSize+flushSize/4), w: w}
}

// Flush writes any buffered output and returns the first write error.
func (o *Writer) Flush() error {
	o.flush()
	return o.err
}

func (o *Writer) flush() {
	if o.w == nil || len(o.buf) == 0 {
		return
	}
	if o.err == nil {
		_, o.err = o.w.Write(o.buf)
	}
	o.buf = o.buf[:0]
}

// maybeFlush passes the buffer on once it is large. Formatters call it at
// line or value boundaries.
func (o *Writer) maybeFlush() {
	if len(o.buf) >= flushSize {
		o.flush()
	}
}

func (o *Writer) str(s string) { o.buf = append(o.buf, s...) }
func (o *Writer) byte(c byte)  { o.buf = append(o.buf, c) }

// spaces is sliced for indentation.
const spaces = "                                                                "

func (o *Writer) indent(n int) {
	for n > len(spaces) {
		o.buf = append(o.buf, spaces...)
		n -= len(spaces)
	}
	o.buf = append(o.buf, spaces[:n]...)
}

// SGR color codes used by the formatters.
const (
	codeKey     = "36"
	codeString  = "32"
	codeNumber  = "33"
	codeBoolean = "35"
	codeNull    = "90"
	codeHint    = "90"
	codeAdded   = "32"
	codeRemoved = "31"
	codeChanged = "33"
)

// open and close surround colored text, like Colorizer.wrap.
func (o *Writer) open(c Colorizer, code string) {
	if c.Enabled {
		o.buf = append(o.buf, "\x1b["...)
		o.buf = append(o.buf, code...)
		o.buf = append(o.buf, 'm')
	}
}

func (o *Writer) close(c Colorizer) {
	if c.Enabled {
		o.buf = append(o.buf, "\x1b[0m"...)
	}
}

func (o *Writer) colored(c Colorizer, code, s string) {
	o.open(c, code)
	o.str(s)
	o.close(c)
}

// quotedKey writes a colored, quoted object key.
func (o *Writer) quotedKey(c Colorizer, key string) {
	o.open(c, codeKey)
	o.buf = parser.AppendQuoted(o.buf, key)
	o.close(c)
}

// primitive writes a colored scalar value (the same text as
// node.StringValue()). It reports false for containers.
func (o *Writer) primitive(c Colorizer, node *parser.Node) bool {
	var code string
	switch node.Type {
	case parser.TypeString:
		code = codeString
	case parser.TypeNumber:
		code = codeNumber
	case parser.TypeBoolean:
		code = codeBoolean
	case parser.TypeNull:
		o.colored(c, codeNull, "null")
		return true
	default:
		return false
	}
	o.open(c, code)
	o.buf = parser.AppendStringValue(o.buf, node)
	o.close(c)
	return true
}
