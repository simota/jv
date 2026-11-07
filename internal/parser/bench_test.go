package parser

import (
	"strconv"
	"strings"
	"testing"
)

// BenchmarkSortKeys measures --sort-keys on 20,000 small records.
func BenchmarkSortKeys(b *testing.B) {
	var sb strings.Builder
	sb.WriteByte('[')
	for i := 0; i < 20000; i++ {
		if i > 0 {
			sb.WriteByte(',')
		}
		sb.WriteString(`{"id":` + strconv.Itoa(i) + `,"name":"x","email":"e","active":true,"score":1,"tags":["a"],"address":{"city":"c","zip":"z","geo":[1,2]},"note":"n"}`)
	}
	sb.WriteByte(']')
	src := sb.String()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		b.StopTimer()
		docs, _ := ParseString(src)
		b.StartTimer()
		SortKeys(docs[0])
	}
}
