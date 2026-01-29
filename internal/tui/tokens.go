package tui

type Tokens struct {
	Colors     ColorTokens
	Spacing    SpacingTokens
	Typography TypographyTokens
	Effects    EffectTokens
}

type ColorTokens struct {
	Key        string
	String     string
	Number     string
	Boolean    string
	Null       string
	TypeHint   string
	SelectedBg string
	SelectedFg string
	Header     string
	Footer     string
	Help       string
}

type SpacingTokens struct {
	Indent    int
	InlineGap int
}

type TypographyTokens struct {
	HeaderBold   bool
	SelectedBold bool
}

type EffectTokens struct {
	Radius int
	Shadow string
}

func DefaultTokens(theme string, colorEnabled bool) Tokens {
	colors := ColorTokens{}
	if colorEnabled {
		colors = ColorTokens{
			Key:        "6",
			String:     "2",
			Number:     "3",
			Boolean:    "5",
			Null:       "8",
			TypeHint:   "8",
			SelectedBg: "4",
			SelectedFg: "0",
			Header:     "8",
			Footer:     "8",
			Help:       "8",
		}
		if theme == "light" {
			colors.SelectedBg = "12"
		}
	}

	return Tokens{
		Colors: colors,
		Spacing: SpacingTokens{
			Indent:    2,
			InlineGap: 1,
		},
		Typography: TypographyTokens{
			HeaderBold:   true,
			SelectedBold: true,
		},
		Effects: EffectTokens{
			Radius: 0,
			Shadow: "none",
		},
	}
}
