package parser

import (
	"bytes"
	"encoding/json"
	"regexp"
	"strings"
	"testing"
)

// encodingJSONQuote is the reference: encoding/json without HTML escaping.
func encodingJSONQuote(s string) string {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	_ = enc.Encode(s)
	return strings.TrimSuffix(buf.String(), "\n")
}

var quoteSeeds = []string{
	"", "plain", `quote " and \ backslash`, "\b\f\n\r\t\x00\x01\x1f\x7f", "<html>&",
	"日本語", "\xe2\x80\xa8\xe2\x80\xa9", "bad \xff\xfe utf8 \xc0\xaf", "emoji 😀", "\xed\xa0\x80", "tail \xe6\x97",
}

func FuzzQuoteMatchesEncodingJSON(f *testing.F) {
	for _, s := range quoteSeeds {
		f.Add(s)
	}
	f.Fuzz(func(t *testing.T, s string) {
		want := encodingJSONQuote(s)
		if got := QuoteString(s); got != want {
			t.Fatalf("QuoteString(%q) = %s, want %s", s, got, want)
		}
		if got := string(AppendQuoted([]byte("x"), s)); got != "x"+want {
			t.Fatalf("AppendQuoted(%q) = %s, want x%s", s, got, want)
		}
	})
}

func TestQuoteMatchesEncodingJSON(t *testing.T) {
	for _, s := range quoteSeeds {
		if got, want := QuoteString(s), encodingJSONQuote(s); got != want {
			t.Errorf("QuoteString(%q) = %s, want %s", s, got, want)
		}
	}
	// every single byte and every BMP code point
	for b := 0; b < 256; b++ {
		s := string([]byte{byte(b)})
		if got, want := QuoteString(s), encodingJSONQuote(s); got != want {
			t.Errorf("QuoteString(%q) = %s, want %s", s, got, want)
		}
	}
	for r := rune(0); r < 0x10000; r++ {
		s := string(r)
		if got, want := QuoteString(s), encodingJSONQuote(s); got != want {
			t.Fatalf("QuoteString(%q) = %s, want %s", s, got, want)
		}
	}
}

var unquoteSeeds = []string{
	"plain", `\"\\\/\b\f\n\r\t`, "\\u00e9\\ud83d\\ude00", "\\ud800", "\\ud800x", "\\udc00", "\\ud800\\u0041",
	"\\ud800\\ud800\\udc00", "\\uD83D\\uDE00", "\\u12", `\x`, `\'`, "\x01", `\`, "\xff\xfe", "ok \xe6\x97 \\n",
	"\\u0000", "\\uFFFF", "\\ud800\\u12", "\\u00E9x\\u2028",
}

// FuzzUnquoteMatchesEncodingJSON checks that the fast string decoder gives
// encoding/json's result, and only reports success for literals that
// encoding/json accepts.
func FuzzUnquoteMatchesEncodingJSON(f *testing.F) {
	for _, s := range unquoteSeeds {
		f.Add(s)
	}
	f.Fuzz(func(t *testing.T, s string) {
		if strings.Contains(s, `"`) {
			return // cannot appear inside a scanned literal
		}
		checkUnquote(t, s)
	})
}

func TestUnquoteMatchesEncodingJSON(t *testing.T) {
	for _, s := range unquoteSeeds {
		checkUnquote(t, s)
	}
}

func checkUnquote(t *testing.T, s string) {
	t.Helper()
	var want string
	wantErr := json.Unmarshal([]byte(`"`+s+`"`), &want)
	got, ok := unquote(s)
	if wantErr != nil {
		if ok {
			t.Fatalf("unquote(%q) accepted %q, encoding/json: %v", s, got, wantErr)
		}
		return
	}
	if !ok || got != want {
		t.Fatalf("unquote(%q) = %q, %v; want %q", s, got, ok, want)
	}
}

func TestIsSimpleKeyMatchesRegexp(t *testing.T) {
	re := regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)
	keys := []string{"", "a", "A", "_", "a1", "1a", "a-b", "a b", "a.b", "日本", "a\n", "\na", "ab_9Z", "9", "a$", "Ω", "a\x00", "__"}
	for c := 0; c < 256; c++ {
		keys = append(keys, string([]byte{byte(c)}), "a"+string([]byte{byte(c)}))
	}
	for _, k := range keys {
		if got, want := isSimpleKey(k), re.MatchString(k); got != want {
			t.Errorf("isSimpleKey(%q) = %v, want %v", k, got, want)
		}
	}
}
