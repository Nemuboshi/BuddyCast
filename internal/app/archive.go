package app

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"archive/zip"
)

func extract(data []byte, root string) error {
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		return err
	}
	if err := os.MkdirAll(root, 0o755); err != nil {
		return err
	}
	cleanRoot, _ := filepath.Abs(root)
	for _, file := range zr.File {
		path := filepath.Join(root, filepath.FromSlash(file.Name))
		abs, _ := filepath.Abs(path)
		if abs != cleanRoot && !strings.HasPrefix(abs, cleanRoot+string(os.PathSeparator)) {
			return fmt.Errorf("unsafe zip path %q", file.Name)
		}
		if file.FileInfo().IsDir() {
			if err := os.MkdirAll(path, 0o755); err != nil {
				return err
			}
			continue
		}
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			return err
		}
		r, err := file.Open()
		if err != nil {
			return err
		}
		out, err := os.Create(path)
		if err != nil {
			r.Close()
			return err
		}
		_, copyErr := io.Copy(out, r)
		closeErr := errors.Join(r.Close(), out.Close())
		if err := errors.Join(copyErr, closeErr); err != nil {
			return err
		}
	}
	return nil
}
