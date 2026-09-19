package app

import (
	"encoding/json"
	"encoding/xml"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestFixtures(t *testing.T) {
	encrypted, err := os.ReadFile(filepath.Join("..", "..", "fixtures", "encrypted.zip"))
	if err != nil {
		t.Fatal(err)
	}
	decrypted := decrypt(encrypted)
	expected, err := os.ReadFile(filepath.Join("..", "..", "fixtures", "decrypted.zip"))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decrypted, expected) {
		t.Fatal("decryption does not match fixture")
	}

	xmlData, err := os.ReadFile(filepath.Join("..", "..", "fixtures", "subtitles_ja.oxk.decrypted"))
	if err != nil {
		t.Fatal(err)
	}
	var root subtitleRoot
	if err := xml.Unmarshal(xmlData, &root); err != nil {
		t.Fatal(err)
	}
	actual := renderASS(root.Lines)
	want, _ := os.ReadFile(filepath.Join("..", "..", "fixtures", "expected.ass"))
	if actual != strings.ReplaceAll(string(want), "\r\n", "\n") {
		t.Fatal("ASS output does not match fixture")
	}

	db, _ := os.ReadFile(filepath.Join("..", "..", "fixtures", "sample.db"))
	report, err := parseDB(db, "sample.db")
	if err != nil {
		t.Fatal(err)
	}
	wantJSON, _ := os.ReadFile(filepath.Join("..", "..", "fixtures", "expected_db.json"))
	var wantReport map[string]any
	if err := json.Unmarshal(wantJSON, &wantReport); err != nil {
		t.Fatal(err)
	}
	actualJSON, _ := json.Marshal(report)
	wantNormalized, _ := json.Marshal(wantReport)
	if string(actualJSON) != string(wantNormalized) {
		t.Fatalf("DB report mismatch\n%s\n%s", actualJSON, wantNormalized)
	}
}
