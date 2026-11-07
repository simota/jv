package parser

import (
	"unicode"
	"unicode/utf16"
	"unicode/utf8"
)

const hexDigits = "0123456789abcdef"

// AppendQuoted appends s as a JSON string literal to dst. The output is the
// same as encoding/json with HTML escaping disabled: `"`, `\` and control
// characters are escaped, invalid UTF-8 becomes \ufffd and U+2028/U+2029 are
// escaped.
func AppendQuoted(dst []byte, s string) []byte {
	dst = append(dst, '"')
	start := 0
	for i := 0; i < len(s); {
		if b := s[i]; b < utf8.RuneSelf {
			if b >= 0x20 && b != '"' && b != '\\' {
				i++
				continue
			}
			dst = append(dst, s[start:i]...)
			switch b {
			case '\\', '"':
				dst = append(dst, '\\', b)
			case '\b':
				dst = append(dst, '\\', 'b')
			case '\f':
				dst = append(dst, '\\', 'f')
			case '\n':
				dst = append(dst, '\\', 'n')
			case '\r':
				dst = append(dst, '\\', 'r')
			case '\t':
				dst = append(dst, '\\', 't')
			default:
				dst = append(dst, '\\', 'u', '0', '0', hexDigits[b>>4], hexDigits[b&0xF])
			}
			i++
			start = i
			continue
		}
		c, size := utf8.DecodeRuneInString(s[i:])
		if c == utf8.RuneError && size == 1 {
			dst = append(dst, s[start:i]...)
			dst = append(dst, `\ufffd`...)
			i++
			start = i
			continue
		}
		if c == '\u2028' || c == '\u2029' {
			dst = append(dst, s[start:i]...)
			dst = append(dst, '\\', 'u', '2', '0', '2', hexDigits[c&0xF])
		} else {
			i += size
			continue
		}
		i += size
		start = i
	}
	dst = append(dst, s[start:]...)
	return append(dst, '"')
}

// unquote decodes the contents of a JSON string literal (without the
// surrounding quotes) the way encoding/json does: invalid UTF-8 is replaced
// by U+FFFD and unpaired surrogates become U+FFFD. It reports false for any
// input that encoding/json would reject (bad escapes, control characters),
// so that the caller can produce encoding/json's error message.
func unquote(s string) (string, bool) {
	b, ok := appendUnquoted(make([]byte, 0, len(s)), s)
	if !ok {
		return "", false
	}
	return string(b), true
}

// appendUnquoted appends the decoded contents of s (see unquote) to b.
func appendUnquoted(b []byte, s string) ([]byte, bool) {
	for r := 0; r < len(s); {
		c := s[r]
		switch {
		case c == '\\':
			r++
			if r >= len(s) {
				return b, false
			}
			switch s[r] {
			case '"', '\\', '/':
				b = append(b, s[r])
				r++
			case 'b':
				b = append(b, '\b')
				r++
			case 'f':
				b = append(b, '\f')
				r++
			case 'n':
				b = append(b, '\n')
				r++
			case 'r':
				b = append(b, '\r')
				r++
			case 't':
				b = append(b, '\t')
				r++
			case 'u':
				rr := getu4(s[r-1:])
				if rr < 0 {
					return b, false
				}
				r += 5
				if utf16.IsSurrogate(rr) {
					// The next escape must be valid JSON even when it does not
					// complete the pair.
					rr1 := getu4(s[r:])
					if dec := utf16.DecodeRune(rr, rr1); dec != unicode.ReplacementChar {
						r += 6
						b = utf8.AppendRune(b, dec)
						break
					}
					rr = unicode.ReplacementChar
				}
				b = utf8.AppendRune(b, rr)
			default:
				return b, false
			}
		case c == '"' || c < ' ':
			return b, false
		case c < utf8.RuneSelf:
			b = append(b, c)
			r++
		default:
			rr, size := utf8.DecodeRuneInString(s[r:])
			r += size
			b = utf8.AppendRune(b, rr)
		}
	}
	return b, true
}

// getu4 decodes \uXXXX from the beginning of s, returning the hex value,
// or it returns -1.
func getu4(s string) rune {
	if len(s) < 6 || s[0] != '\\' || s[1] != 'u' {
		return -1
	}
	var r rune
	for _, c := range []byte(s[2:6]) {
		switch {
		case '0' <= c && c <= '9':
			c = c - '0'
		case 'a' <= c && c <= 'f':
			c = c - 'a' + 10
		case 'A' <= c && c <= 'F':
			c = c - 'A' + 10
		default:
			return -1
		}
		r = r*16 + rune(c)
	}
	return r
}
