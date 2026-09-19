package app

import (
	"encoding/xml"
	"fmt"
	"math"
	"strings"
)

type subtitleRoot struct {
	Lines []subtitleLine `xml:"SubData>SubLineData"`
}
type subtitleLine struct {
	Start     float64 `xml:"Start"`
	End       float64 `xml:"End"`
	Alignment string  `xml:"Alignment"`
	Text      string  `xml:"Text"`
}

const assHeader = `[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes
YCbCr Matrix: TV.709

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Noto Sans JP,54,&H00FFFFFF,&H0000FFFF,&H00000000,&H64000000,0,0,0,0,100,100,0,0,1,2,0,2,60,60,40,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
`

func parseSubtitles(data []byte) ([]subtitleLine, error) {
	var root subtitleRoot
	if err := xml.Unmarshal(trimBOM(data), &root); err != nil {
		return nil, err
	}
	return root.Lines, nil
}
func trimBOM(b []byte) []byte {
	if len(b) >= 3 && b[0] == 0xef && b[1] == 0xbb && b[2] == 0xbf {
		return b[3:]
	}
	return b
}

func renderASS(lines []subtitleLine) string {
	var out strings.Builder
	out.WriteString(assHeader)
	for _, line := range lines {
		fmt.Fprintf(&out, "\nDialogue: 0,%s,%s,Default,,0,0,0,,%s%s", assTime(line.Start), assTime(line.End), alignment(line.Alignment), ruby(line.Text))
	}
	return out.String()
}
func renderSRT(lines []subtitleLine) string {
	var out strings.Builder
	for i, line := range lines {
		fmt.Fprintf(&out, "%d\n%s --> %s\n%s\n\n", i+1, srtTime(line.Start), srtTime(line.End), ruby(line.Text))
	}
	return out.String()
}
func alignment(s string) string {
	switch s {
	case "bottom":
		return `{\an2}`
	case "bottom_left":
		return `{\an1}`
	case "bottom_right":
		return `{\an3}`
	}
	return ""
}
func ruby(s string) string {
	for {
		a := strings.Index(s, `{\r1}`)
		if a < 0 {
			return s
		}
		b := strings.Index(s[a+5:], `{\r2}`)
		if b < 0 {
			return s
		}
		b += a + 5
		c := strings.Index(s[b+5:], `{\r0}`)
		if c < 0 {
			return s
		}
		c += b + 5
		s = s[:a] + s[a+5:b] + "(" + s[b+5:c] + ")" + s[c+5:]
	}
}
func assTime(v float64) string {
	n := int64(math.Round(v * 100))
	return fmt.Sprintf("%d:%02d:%02d.%02d", n/360000, n/6000%60, n/100%60, n%100)
}
func srtTime(v float64) string {
	n := int64(math.Round(v * 1000))
	return fmt.Sprintf("%02d:%02d:%02d,%03d", n/3600000, n/60000%60, n/1000%60, n%1000)
}
