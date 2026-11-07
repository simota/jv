package pipe

import (
	"bytes"
	"errors"
	"strings"
	"testing"

	"github.com/simota/jv/internal/parser"
)

// chunkRecorder records the size of every write.
type chunkRecorder struct {
	bytes.Buffer
	writes []int
}

func (c *chunkRecorder) Write(p []byte) (int, error) {
	c.writes = append(c.writes, len(p))
	return c.Buffer.Write(p)
}

// Every formatter streams: output reaches the writer in bounded chunks, even
// for a single large document on one line (--compact), and the text is the
// same as Format returns.
func TestFormattersStreamInBoundedChunks(t *testing.T) {
	docs, err := parser.ParseBytes(benchRecords(3000))
	if err != nil {
		t.Fatal(err)
	}
	root := docs[0]
	formatters := map[string]Formatter{
		"pretty":  NewPrettyFormatter(true),
		"compact": NewCompactFormatter(true),
		"flat":    NewFlatFormatter(true),
		"typed":   NewTypedFormatter(true),
	}
	for name, f := range formatters {
		var rec chunkRecorder
		out := NewWriter(&rec)
		f.FormatTo(out, root)
		if err := out.Flush(); err != nil {
			t.Fatal(err)
		}
		if rec.String() != f.Format(root) {
			t.Errorf("%s: FormatTo and Format differ", name)
		}
		if len(rec.writes) < 2 {
			t.Errorf("%s: output was not streamed (%d writes)", name, len(rec.writes))
		}
		for _, n := range rec.writes {
			if n > 2*flushSize {
				t.Errorf("%s: write of %d bytes exceeds the chunk bound", name, n)
			}
		}
	}
}

type failingWriter struct{ n int }

func (f *failingWriter) Write(p []byte) (int, error) {
	f.n++
	return 0, errors.New("disk full")
}

// The first write error is reported by Flush and nothing more is written.
func TestWriterKeepsFirstError(t *testing.T) {
	fw := &failingWriter{}
	out := NewWriter(fw)
	big := strings.Repeat("x", flushSize)
	for i := 0; i < 3; i++ {
		out.str(big)
		out.maybeFlush()
	}
	if err := out.Flush(); err == nil || err.Error() != "disk full" {
		t.Fatalf("err = %v", err)
	}
	if fw.n != 1 {
		t.Fatalf("writes after the error: %d", fw.n)
	}
}
