package app

import (
	"bytes"
	"context"
	"encoding/json"
	"encoding/xml"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/Nemuboshi/buddycast/internal/cli"
	"github.com/Nemuboshi/buddycast/internal/progress"
	"github.com/Nemuboshi/buddycast/internal/termio"
)

var logger = newCLILogger(os.Stdout)

type cliLogger struct {
	out      *os.File
	useColor bool
}

func newCLILogger(out *os.File) *cliLogger {
	return &cliLogger{out: out, useColor: termio.IsTTY(out)}
}
func (l *cliLogger) Info(msg string, attrs ...any)  { l.log("INFO", msg, attrs...) }
func (l *cliLogger) Warn(msg string, attrs ...any)  { l.log("WARN", msg, attrs...) }
func (l *cliLogger) Error(msg string, attrs ...any) { l.log("ERROR", msg, attrs...) }
func (l *cliLogger) log(level, msg string, attrs ...any) {
	lvl := level
	if l.useColor {
		switch level {
		case "INFO":
			lvl = "\x1b[36mINFO\x1b[0m"
		case "WARN":
			lvl = "\x1b[33mWARN\x1b[0m"
		case "ERROR":
			lvl = "\x1b[31mERROR\x1b[0m"
		}
	}
	var b strings.Builder
	b.WriteString(time.Now().Format("01-02 15:04:05"))
	b.WriteString(" ")
	b.WriteString(lvl)
	b.WriteString(" ")
	b.WriteString(msg)
	for i := 0; i+1 < len(attrs); i += 2 {
		fmt.Fprintf(&b, " %v=%v", attrs[i], attrs[i+1])
	}
	fmt.Fprintln(l.out, b.String())
}

func Run(ctx context.Context, cmd cli.Command) error {
	switch cmd.Name {
	case "getinfo":
		return getinfo(ctx, cmd)
	case "fetch":
		return fetch(ctx, cmd)
	default:
		return fmt.Errorf("unknown command %q", cmd.Name)
	}
}

func getinfo(ctx context.Context, cmd cli.Command) error {
	logger.Info("fetching contents list")
	raw, list, err := fetchList(ctx, cmd.Timeout)
	if err != nil {
		return err
	}
	if cmd.Save != "" {
		if err := writeFile(cmd.Save, raw); err != nil {
			return err
		}
		logger.Info("saved contents", "path", cmd.Save)
	}
	items := displayItems(list)
	if cmd.Limit > 0 && len(items) > cmd.Limit {
		items = items[:cmd.Limit]
	}
	fmt.Printf("%-24s  %-10s  %s\n", "ASSET_ID", "SIZE", "TITLE")
	fmt.Printf("%-24s  %-10s  %s\n", strings.Repeat("-", 24), strings.Repeat("-", 10), strings.Repeat("-", 60))
	for _, item := range items {
		fmt.Printf("%-24s  %-10s  %s\n", item.Contents.ETags, item.Contents.Size, strings.ReplaceAll(item.Title, "<br/>", ""))
	}
	return nil
}

func fetch(ctx context.Context, cmd cli.Command) error {
	var raw []byte
	var list contentList
	var err error
	if cmd.Offline {
		raw, err = os.ReadFile(filepath.Join("downloads", "contents.json"))
		if err == nil {
			err = json.Unmarshal(raw, &list)
		}
	} else {
		logger.Info("fetching contents list")
		raw, list, err = fetchList(ctx, cmd.Timeout)
	}
	if err != nil {
		return fmt.Errorf("load contents: %w", err)
	}

	var selected *item
	for i := range list.Contents.DeliveryContents {
		if visible(list.Contents.DeliveryContents[i]) && list.Contents.DeliveryContents[i].Contents.ETags == cmd.AssetID {
			selected = &list.Contents.DeliveryContents[i]
			break
		}
	}
	if selected == nil {
		return fmt.Errorf("asset id not found: %s", cmd.AssetID)
	}
	logger.Info("selected", "asset_id", cmd.AssetID, "title", strings.ReplaceAll(selected.Title, "<br/>", " "))
	if err := os.MkdirAll(cmd.Out, 0o755); err != nil {
		return err
	}
	if err := writeFile(filepath.Join(cmd.Out, "contents.json"), raw); err != nil {
		return err
	}

	encryptedPath := filepath.Join(cmd.Out, cmd.AssetID+".encrypted.zip")
	var encrypted []byte
	if cmd.Offline {
		encrypted, err = os.ReadFile(filepath.Join("downloads", cmd.AssetID+".encrypted.zip"))
	} else {
		encrypted, err = download(ctx, absoluteURL(selected.Contents.URL), cmd.Timeout, cmd.AssetID)
	}
	if err != nil {
		return fmt.Errorf("download package: %w", err)
	}
	if err := writeFile(encryptedPath, encrypted); err != nil {
		return err
	}

	logger.Info("decrypting package", "asset_id", cmd.AssetID, "bytes", len(encrypted))
	tty := termio.IsTTY(os.Stdout)
	decrypted := make([]byte, len(encrypted))
	const chunk = 8192
	for start := 0; start < len(encrypted); start += chunk {
		end := min(start+chunk, len(encrypted))
		for i := start; i < end; i++ {
			decrypted[i] = decryptionMap[encrypted[i]]
		}
		if tty {
			progress.Render(os.Stdout, "decrypt", int64(end), int64(len(encrypted)))
		}
	}
	if tty {
		fmt.Println()
	}
	if !bytes.HasPrefix(decrypted, []byte("PK")) {
		return errors.New("decrypted package is not a zip archive")
	}
	zipPath := filepath.Join(cmd.Out, cmd.AssetID+".zip")
	if err := writeFile(zipPath, decrypted); err != nil {
		return err
	}

	workDir := filepath.Join(cmd.Out, safeName(selected.Title, cmd.AssetID))
	if err := os.RemoveAll(workDir); err != nil {
		return err
	}
	logger.Info("extracting", "dir", workDir)
	if err := extract(decrypted, workDir); err != nil {
		return err
	}
	logger.Info("processing files")
	outputs, reports, err := postprocess(workDir, cmd.SRT)
	if err != nil {
		return err
	}
	if !cmd.KeepEncryptedZip {
		_ = os.Remove(encryptedPath)
	}
	logger.Info("output ready", "dir", workDir, "subtitles", len(outputs), "db_reports", len(reports))
	return nil
}

func postprocess(root string, srt bool) ([]string, []string, error) {
	var candidates []string
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err == nil && !entry.IsDir() {
			candidates = append(candidates, path)
		}
		return err
	})
	if err != nil {
		return nil, nil, err
	}
	tty := termio.IsTTY(os.Stdout)
	var subtitles, reports []string
	for i, path := range candidates {
		data, err := os.ReadFile(path)
		if err != nil {
			return nil, nil, err
		}
		lower := strings.ToLower(path)
		if needsDecryption(lower, data) {
			data = decrypt(data)
			if err := os.WriteFile(path, data, 0o644); err != nil {
				return nil, nil, err
			}
		}
		if strings.HasSuffix(lower, ".oxk") || strings.HasSuffix(lower, ".oxk.decrypted") {
			var root subtitleRoot
			if xml.Unmarshal(bytes.TrimPrefix(data, []byte{0xef, 0xbb, 0xbf}), &root) == nil {
				base := strings.TrimSuffix(path, ".decrypted")
				ass := strings.TrimSuffix(base, filepath.Ext(base)) + ".ass"
				if err := os.WriteFile(ass, []byte(renderASS(root.Lines)), 0o644); err != nil {
					return nil, nil, err
				}
				subtitles = append(subtitles, ass)
				if srt {
					out := strings.TrimSuffix(base, filepath.Ext(base)) + ".srt"
					if err := os.WriteFile(out, []byte(renderSRT(root.Lines)), 0o644); err != nil {
						return nil, nil, err
					}
					subtitles = append(subtitles, out)
				}
			}
		}
		if strings.HasSuffix(lower, ".db") {
			report, err := parseDB(data, filepath.Base(path))
			if err != nil {
				return nil, nil, err
			}
			out := path + ".json"
			encoded, _ := json.MarshalIndent(report, "", "  ")
			if err := os.WriteFile(out, encoded, 0o644); err != nil {
				return nil, nil, err
			}
			reports = append(reports, out)
		}
		if tty {
			progress.Render(os.Stdout, "process", int64(i+1), int64(len(candidates)))
		}
	}
	if tty {
		fmt.Println()
	}
	return subtitles, reports, nil
}

func needsDecryption(path string, data []byte) bool {
	if strings.HasSuffix(path, ".oxk") || strings.HasSuffix(path, ".oxk.decrypted") {
		return !bytes.Contains(data[:min(len(data), 128)], []byte("<"))
	}
	if strings.HasSuffix(path, ".db") {
		return len(data) < 102
	}
	if strings.Contains(path, string(os.PathSeparator)+"img"+string(os.PathSeparator)) || strings.Contains(path, string(os.PathSeparator)+"pict"+string(os.PathSeparator)) {
		return !imageHeader(data)
	}
	return false
}
func imageHeader(b []byte) bool {
	return bytes.HasPrefix(b, []byte("\x89PNG\r\n\x1a\n")) || bytes.HasPrefix(b, []byte("GIF8")) || bytes.HasPrefix(b, []byte{0xff, 0xd8, 0xff})
}
func decrypt(data []byte) []byte {
	out := make([]byte, len(data))
	for i, b := range data {
		out[i] = decryptionMap[b]
	}
	return out
}

func safeName(title, fallback string) string {
	title = strings.ReplaceAll(title, "<br/>", " ")
	title = strings.Map(func(r rune) rune {
		if strings.ContainsRune(`<>:"/\\|?*`, r) {
			return '_'
		}
		if r == '\n' || r == '\r' || r == '\t' {
			return ' '
		}
		return r
	}, title)
	title = strings.Join(strings.Fields(title), " ")
	if title == "" {
		return fallback
	}
	return title
}
func writeFile(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}

var decryptionMap = [256]byte{28, 42, 12, 24, 36, 125, 3, 46, 123, 15, 7, 39, 40, 13, 8, 93, 44, 95, 33, 34, 126, 127, 61, 0, 16, 23, 4, 14, 92, 6, 1, 19, 32, 124, 20, 35, 27, 37, 38, 63, 9, 59, 62, 94, 29, 10, 31, 21, 79, 99, 75, 65, 87, 120, 82, 118, 53, 57, 30, 18, 43, 26, 11, 5, 25, 97, 49, 69, 48, 114, 84, 116, 85, 55, 121, 122, 108, 52, 51, 73, 76, 113, 70, 83, 81, 105, 119, 78, 88, 89, 54, 45, 22, 41, 96, 64, 60, 110, 80, 106, 109, 98, 115, 66, 77, 72, 56, 100, 74, 107, 90, 71, 102, 68, 101, 50, 117, 111, 103, 86, 112, 104, 67, 17, 58, 91, 2, 47, 138, 189, 232, 212, 135, 173, 151, 145, 201, 194, 253, 163, 245, 235, 140, 187, 251, 236, 174, 131, 199, 221, 195, 165, 144, 186, 172, 141, 168, 148, 230, 209, 247, 192, 190, 249, 207, 181, 244, 231, 254, 178, 255, 130, 166, 154, 197, 205, 156, 237, 241, 223, 170, 164, 220, 224, 136, 134, 188, 246, 157, 213, 183, 177, 143, 193, 252, 137, 155, 180, 196, 182, 128, 146, 203, 238, 132, 162, 210, 200, 211, 158, 161, 243, 160, 152, 149, 159, 185, 202, 248, 150, 242, 239, 222, 240, 153, 225, 167, 227, 228, 184, 229, 176, 233, 218, 198, 175, 206, 171, 216, 204, 139, 219, 191, 129, 234, 226, 250, 133, 179, 142, 215, 217, 208, 214, 147, 169}
