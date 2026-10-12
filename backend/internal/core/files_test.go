package core

import (
	"strings"
	"testing"
)

func TestValidateFileAllowlist(t *testing.T) {
	cases := []struct {
		media, filename string
	}{
		{"application/pdf", "file.pdf"},
		{"application/msword", "file.doc"},
		{"application/vnd.openxmlformats-officedocument.wordprocessingml.document", "file.docx"},
		{"application/vnd.ms-excel", "file.xls"},
		{"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "file.xlsx"},
		{"text/csv", "file.csv"},
		{"text/plain", "file.txt"},
		{"image/jpeg", "file.jpg"},
		{"image/jpeg", "file.jpeg"},
		{"image/png", "file.png"},
	}
	for _, tc := range cases {
		t.Run(tc.filename, func(t *testing.T) {
			p := FileRequest{Filename: tc.filename, MediaType: tc.media, Size: 10, Checksum: strings.Repeat("a", 64), Version: 0}
			if e := ValidateFile(p); e != nil {
				t.Fatalf("expected allowlisted file: %v", e)
			}
		})
	}
	for _, tc := range []struct {
		name string
		edit func(*FileRequest)
	}{
		{"extension mismatch", func(p *FileRequest) { p.Filename = "file.svg" }},
		{"unlisted media", func(p *FileRequest) { p.MediaType = "image/svg+xml" }},
		{"bad team ID", func(p *FileRequest) { p.TeamID = "0" }},
		{"path filename", func(p *FileRequest) { p.Filename = "../file.pdf" }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			p := FileRequest{Filename: "file.pdf", MediaType: "application/pdf", Size: 10, Checksum: strings.Repeat("a", 64)}
			tc.edit(&p)
			if e := ValidateFile(p); e == nil {
				t.Fatal("expected validation rejection")
			}
		})
	}
}

func TestValidateFileContentBySignature(t *testing.T) {
	valid := []struct {
		media  string
		prefix []byte
	}{
		{"application/pdf", []byte("%PDF-1.7 synthetic")},
		{"image/png", []byte("\x89PNG\r\n\x1a\nsynthetic")},
		{"image/jpeg", []byte{0xff, 0xd8, 0xff, 0xe0}},
		{"application/msword", []byte("\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1synthetic")},
		{"application/vnd.ms-excel", []byte("\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1synthetic")},
		{"application/vnd.openxmlformats-officedocument.wordprocessingml.document", []byte("PK\x03\x04synthetic")},
		{"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", []byte("PK\x03\x04synthetic")},
		{"text/csv", []byte("name,amount\nitem,12\n")},
		{"text/plain", []byte("Texto sintético\n")},
	}
	for _, tc := range valid {
		t.Run(tc.media, func(t *testing.T) {
			if !ValidateFileContent(tc.media, tc.prefix) {
				t.Fatal("expected recognized content signature")
			}
		})
	}
	invalid := []struct {
		media  string
		prefix []byte
	}{
		{"application/pdf", []byte("not pdf")},
		{"image/png", []byte("<svg xmlns=\"http://www.w3.org/2000/svg\"/>")},
		{"application/msword", []byte("PK\x03\x04wrong format")},
		{"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", []byte("not zip")},
		{"text/csv", []byte("\xff\xfe\xfd")},
		{"text/plain", []byte("<html><script>active</script></html>")},
	}
	for _, tc := range invalid {
		t.Run(tc.media+"-invalid", func(t *testing.T) {
			if ValidateFileContent(tc.media, tc.prefix) {
				t.Fatal("expected content rejection")
			}
		})
	}
}
