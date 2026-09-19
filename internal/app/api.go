package app

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/Nemuboshi/buddycast/internal/progress"
	"github.com/Nemuboshi/buddycast/internal/termio"
)

const (
	baseURL  = "https://cms.palabra.jp"
	listPath = "/masc/encrypt/list"
	user     = "UDeC4PTB"
	password = "iqKJwKZJ"
)

type contentList struct {
	Contents struct {
		DeliveryContents []item `json:"deliveryContents"`
	} `json:"contents"`
}
type item struct {
	ID, Title string
	Tags      []string
	Contents  struct{ URL, ETags, Size string }
}

func fetchList(ctx context.Context, timeout float64) ([]byte, contentList, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, baseURL+listPath, nil)
	if err != nil {
		return nil, contentList{}, err
	}
	req.Header.Set("Authorization", "Basic "+base64.StdEncoding.EncodeToString([]byte(user+":"+password)))
	req.Header.Set("User-Agent", "UDCast/5.3.2 Go port")
	client := &http.Client{Timeout: time.Duration(timeout * float64(time.Second))}
	var last error
	for attempt := 0; attempt < 4; attempt++ {
		resp, requestErr := client.Do(req)
		if requestErr == nil {
			raw, readErr := io.ReadAll(resp.Body)
			resp.Body.Close()
			if readErr == nil && resp.StatusCode >= 200 && resp.StatusCode < 300 {
				var list contentList
				if err := json.Unmarshal(raw, &list); err != nil {
					return nil, list, err
				}
				return raw, list, nil
			}
			if readErr != nil {
				last = readErr
			} else {
				last = fmt.Errorf("HTTP %s", resp.Status)
			}
		} else {
			last = requestErr
		}
		if attempt < 3 {
			time.Sleep(time.Second)
		}
	}
	return nil, contentList{}, last
}

func download(ctx context.Context, url string, timeout float64, assetID string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", "UDCast/5.3.2 Go port")
	resp, err := (&http.Client{Timeout: time.Duration(timeout * float64(time.Second))}).Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("HTTP %s", resp.Status)
	}
	tty := termio.IsTTY(os.Stdout) && resp.ContentLength > 0
	buf := make([]byte, 32*1024)
	var out bytes.Buffer
	var done int64
	for {
		n, readErr := resp.Body.Read(buf)
		if n > 0 {
			out.Write(buf[:n])
			done += int64(n)
			if tty {
				progress.Render(os.Stdout, "download", done, resp.ContentLength)
			}
		}
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			return nil, readErr
		}
	}
	if tty {
		fmt.Println()
	}
	return out.Bytes(), nil
}

func displayItems(list contentList) []item {
	var result []item
	for _, item := range list.Contents.DeliveryContents {
		if visible(item) {
			result = append(result, item)
		}
	}
	return result
}
func visible(item item) bool {
	size, _ := strconv.ParseInt(item.Contents.Size, 10, 64)
	hasSub := false
	for _, tag := range item.Tags {
		if strings.EqualFold(tag, "sub") {
			hasSub = true
		}
	}
	return hasSub && !strings.Contains(item.Title, "調整中") && size > 100000
}
func absoluteURL(path string) string {
	if strings.HasPrefix(path, "http://") || strings.HasPrefix(path, "https://") {
		return path
	}
	return baseURL + path
}
