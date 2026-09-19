package progress

import (
	"fmt"
	"io"
	"strings"
)

// Render draws a single-line count progress bar in GoodbyeMovie style.
// Non-TTY callers should skip the call entirely.
func Render(out io.Writer, label string, done, total int64) {
	const width = 30
	if total <= 0 {
		return
	}
	filled := done * width / total
	if filled > width {
		filled = width
	}
	bar := strings.Repeat("=", int(filled)) + strings.Repeat("-", width-int(filled))
	fmt.Fprintf(out, "\r%s [%s] %d/%d", label, bar, done, total)
}
