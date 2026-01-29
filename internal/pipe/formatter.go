package pipe

import "github.com/simota/jv/internal/parser"

type Formatter interface {
	Format(root *parser.Node) string
}

type Colorizer struct {
	Enabled bool
}

func (c Colorizer) wrap(code int, s string) string {
	if !c.Enabled {
		return s
	}
	return "\x1b[" + itoa(code) + "m" + s + "\x1b[0m"
}

func (c Colorizer) Key(s string) string    { return c.wrap(36, s) }
func (c Colorizer) String(s string) string { return c.wrap(32, s) }
func (c Colorizer) Number(s string) string { return c.wrap(33, s) }
func (c Colorizer) Boolean(s string) string {
	return c.wrap(35, s)
}
func (c Colorizer) Null(s string) string     { return c.wrap(90, s) }
func (c Colorizer) TypeHint(s string) string { return c.wrap(90, s) }

func itoa(v int) string {
	if v == 0 {
		return "0"
	}
	buf := [32]byte{}
	i := len(buf)
	for v > 0 {
		i--
		buf[i] = byte('0' + v%10)
		v /= 10
	}
	return string(buf[i:])
}
