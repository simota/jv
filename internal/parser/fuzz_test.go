package parser

import (
	"bytes"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

// FuzzParseMatchesEncodingJSON checks that the hand-written scanner accepts
// exactly the single documents encoding/json accepts and yields equal values.
func FuzzParseMatchesEncodingJSON(f *testing.F) {
	seeds := []string{
		`{}`, `[]`, `0`, `-0.5e+10`, `"a\u00e9\ud83d\ude00\n"`, `{"a":[1,{"b":null}],"c":true}`,
		`{"a":1,"a":2}`, `[1,]`, `{"a" 1}`, `01`, `1.`, `-`, `"\x01"`, `tru`, `nul`, "\"\xff\"",
		`{"k":"v"}  `, ` [ 1 , 2 ] `, `[1e5, 1E-5, 0.0]`, `"\/"`, `{"":""}`,
	}
	for _, s := range seeds {
		f.Add([]byte(s))
	}
	f.Fuzz(func(t *testing.T, data []byte) {
		docs, err := ParseBytes(data)
		valid := json.Valid(data)
		if valid {
			if err != nil || len(docs) != 1 {
				t.Fatalf("valid JSON rejected: %q: %v (docs=%d)", data, err, len(docs))
			}
			var want any
			dec := json.NewDecoder(bytes.NewReader(data))
			dec.UseNumber()
			if err := dec.Decode(&want); err != nil {
				t.Fatal(err)
			}
			var got any
			dec = json.NewDecoder(strings.NewReader(Marshal(docs[0], "")))
			dec.UseNumber()
			if err := dec.Decode(&got); err != nil {
				t.Fatalf("Marshal produced invalid JSON for %q: %v", data, err)
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("value mismatch for %q:\n got %#v\nwant %#v", data, got, want)
			}
			return
		}
		if err == nil && len(docs) == 1 {
			t.Fatalf("invalid JSON accepted as single document: %q", data)
		}
	})
}

func TestDeepNestingIsRejected(t *testing.T) {
	deep := strings.Repeat("[", maxNesting+1) + strings.Repeat("]", maxNesting+1)
	if _, err := ParseBytes([]byte(deep)); err == nil || !strings.Contains(err.Error(), "max nesting") {
		t.Fatalf("expected nesting error, got %v", err)
	}
	ok := strings.Repeat("[", 100) + strings.Repeat("]", 100)
	if _, err := ParseBytes([]byte(ok)); err != nil {
		t.Fatal(err)
	}
}
