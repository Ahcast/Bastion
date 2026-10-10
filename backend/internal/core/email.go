package core

import (
	"encoding/hex"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

const MaxEmailAttachmentBytes = 10 * 1024 * 1024

// ValidateEmail checks transport input, not the commercial authority to send it.
// No transport adapter resolves filenames, URLs or business entities itself.
func ValidateEmail(e Email) error {
	to := e.To
	if e.Recipient != "" {
		if len(to) != 0 {
			return Invalid
		}
		to = []string{e.Recipient}
	}
	if len(to) == 0 || len(to)+len(e.CC)+len(e.BCC) > 20 {
		return Invalid
	}
	seen := map[string]bool{}
	for _, list := range [][]string{to, e.CC, e.BCC} {
		for _, address := range list {
			key := strings.ToLower(address)
			if !ValidEmail(address) || strings.IndexFunc(address, unicode.IsControl) >= 0 || seen[key] {
				return Invalid
			}
			seen[key] = true
		}
	}
	if !utf8.ValidString(e.Subject) || utf8.RuneCountInString(e.Subject) < 1 || utf8.RuneCountInString(e.Subject) > 200 || strings.IndexFunc(e.Subject, unicode.IsControl) >= 0 {
		return Invalid
	}
	if !utf8.ValidString(e.Body) || len(e.Body) > 100000 || strings.IndexFunc(e.Body, func(r rune) bool {
		return unicode.IsControl(r) && r != '\t' && r != '\r' && r != '\n'
	}) >= 0 {
		return Invalid
	}
	const suffix = "@bastion.castilloit.net>"
	if !strings.HasPrefix(e.MessageID, "<") || !strings.HasSuffix(e.MessageID, suffix) {
		return Invalid
	}
	id := strings.TrimSuffix(strings.TrimPrefix(e.MessageID, "<"), suffix)
	if len(id) != 32 {
		return Invalid
	}
	if _, err := hex.DecodeString(id); err != nil {
		return Invalid
	}
	if len(e.Attachments) > 10 {
		return Invalid
	}
	total := 0
	seen = map[string]bool{}
	for _, file := range e.Attachments {
		total += len(file.Data)
		key := strings.ToLower(file.Filename)
		if total > MaxEmailAttachmentBytes || seen[key] || strings.IndexFunc(file.Filename, unicode.IsControl) >= 0 {
			return Invalid
		}
		if err := ValidateFile(FileRequest{Filename: file.Filename, MediaType: file.MediaType, Size: int64(len(file.Data)), Checksum: security.Hash(string(file.Data))}); err != nil {
			return err
		}
		seen[key] = true
	}
	return nil
}
