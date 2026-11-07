package pipe

import (
	"strconv"
	"strings"
	"testing"

	"github.com/simota/jv/internal/parser"
)

// benchRecords builds an API-like JSON array with n records (the same shape
// as rust/examples/gen_bench_data.rs).
func benchRecords(n int) []byte {
	var b strings.Builder
	b.WriteByte('[')
	for i := 0; i < n; i++ {
		if i > 0 {
			b.WriteString(",\n")
		}
		s := strconv.Itoa((i * 7919) % 30000)
		b.WriteString(`{"id":` + strconv.Itoa(i) + `,"name":"user-` + s + `","email":"user` + strconv.Itoa(i) +
			`@example.com","active":` + strconv.FormatBool(i%2 == 0) + `,"score":` + s + `.5,"tags":["t` +
			strconv.Itoa(i%10) + `","t3","日本"],"address":{"city":"City ` + strconv.Itoa(i%30) + `","zip":"0` + s +
			`","geo":[34.5,-34.25]},"note":"line\nwith \"escapes\" ` + strconv.Itoa(i%1000) + `"}`)
	}
	b.WriteByte(']')
	return []byte(b.String())
}

func benchParse(b *testing.B, data []byte) *parser.Node {
	b.Helper()
	docs, err := parser.ParseBytes(data)
	if err != nil {
		b.Fatal(err)
	}
	return docs[0]
}

func BenchmarkParse(b *testing.B) {
	data := benchRecords(20000)
	b.SetBytes(int64(len(data)))
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		benchParse(b, data)
	}
}

func benchFormat(b *testing.B, f Formatter) {
	root := benchParse(b, benchRecords(20000))
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = f.Format(root)
	}
}

func BenchmarkPretty(b *testing.B)      { benchFormat(b, NewPrettyFormatter(false)) }
func BenchmarkPrettyColor(b *testing.B) { benchFormat(b, NewPrettyFormatter(true)) }
func BenchmarkCompact(b *testing.B)     { benchFormat(b, NewCompactFormatter(false)) }
func BenchmarkFlat(b *testing.B)        { benchFormat(b, NewFlatFormatter(false)) }
func BenchmarkTyped(b *testing.B)       { benchFormat(b, NewTypedFormatter(false)) }
func BenchmarkSchema(b *testing.B)      { benchFormat(b, NewSchemaFormatter(false)) }
