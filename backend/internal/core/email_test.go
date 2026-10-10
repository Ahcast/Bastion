package core

import (
	"fmt"
	"strings"
	"testing"
)

func validTransportEmail() Email {
	return Email{To: []string{"customer@example.test"}, Subject: "Cotización válida", Body: "Texto público\nSin costos internos.", MessageID: "<0123456789abcdef0123456789abcdef@bastion.castilloit.net>"}
}
func TestEmailValidationRejectsUnsafeTransportInput(t *testing.T) {
	cases := map[string]func(*Email){
		"no recipient":           func(e *Email) { e.To = nil },
		"ambiguous recipient":    func(e *Email) { e.Recipient = "other@example.test" },
		"recipient injection":    func(e *Email) { e.To[0] = "customer@example.test\r\nBcc: hidden@example.test" },
		"display name":           func(e *Email) { e.To[0] = "Customer <customer@example.test>" },
		"invalid cc":             func(e *Email) { e.CC = []string{"invalid"} },
		"unicode bcc":            func(e *Email) { e.BCC = []string{"cliente@ejémplo.test"} },
		"duplicate across lists": func(e *Email) { e.BCC = []string{"CUSTOMER@example.test"} },
		"recipient limit": func(e *Email) {
			for i := 0; i < 20; i++ {
				e.CC = append(e.CC, fmt.Sprintf("cc%d@example.test", i))
			}
		},
		"empty subject":        func(e *Email) { e.Subject = "" },
		"subject injection":    func(e *Email) { e.Subject = "Quote\r\nBcc: stolen@example.test" },
		"subject control":      func(e *Email) { e.Subject = "Quote\x00" },
		"subject length":       func(e *Email) { e.Subject = strings.Repeat("ó", 201) },
		"invalid utf8 subject": func(e *Email) { e.Subject = string([]byte{255}) },
		"body limit":           func(e *Email) { e.Body = strings.Repeat("a", 100001) },
		"body control":         func(e *Email) { e.Body = "public\x00private" },
		"invalid utf8 body":    func(e *Email) { e.Body = string([]byte{255}) },
		"message id injection": func(e *Email) { e.MessageID += "\r\nBcc: stolen@example.test" },
		"message id random":    func(e *Email) { e.MessageID = "<wrong@bastion.castilloit.net>" },
		"message id domain":    func(e *Email) { e.MessageID = "<0123456789abcdef0123456789abcdef@example.test>" },
		"message id nonhex":    func(e *Email) { e.MessageID = "<zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz@bastion.castilloit.net>" },
		"attachment traversal": func(e *Email) {
			e.Attachments = []EmailAttachment{{"../quote.pdf", "application/pdf", []byte("synthetic")}}
		},
		"attachment control": func(e *Email) {
			e.Attachments = []EmailAttachment{{"quote\t.pdf", "application/pdf", []byte("synthetic")}}
		},
		"attachment unsupported": func(e *Email) {
			e.Attachments = []EmailAttachment{{"quote.exe", "application/octet-stream", []byte("synthetic")}}
		},
		"attachment empty": func(e *Email) { e.Attachments = []EmailAttachment{{"quote.pdf", "application/pdf", nil}} },
		"attachment duplicate": func(e *Email) {
			e.Attachments = []EmailAttachment{{"quote.pdf", "application/pdf", []byte("synthetic")}, {"QUOTE.pdf", "application/pdf", []byte("other")}}
		},
		"attachment count": func(e *Email) {
			for i := 0; i < 11; i++ {
				e.Attachments = append(e.Attachments, EmailAttachment{fmt.Sprintf("file%d.txt", i), "text/plain", []byte("synthetic")})
			}
		},
		"attachment total size": func(e *Email) {
			e.Attachments = []EmailAttachment{{"a.txt", "text/plain", make([]byte, MaxEmailAttachmentBytes/2)}, {"b.txt", "text/plain", make([]byte, MaxEmailAttachmentBytes/2+1)}}
		},
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			e := validTransportEmail()
			mutate(&e)
			if ValidateEmail(e) != Invalid {
				t.Fatal("unsafe input accepted")
			}
		})
	}
}
func TestEmailValidationSupportsLegacyAndBoundaries(t *testing.T) {
	e := validTransportEmail()
	e.Subject = strings.Repeat("ó", 200)
	e.Body = "\tTexto con acentos: acción\r\n" + strings.Repeat("a", 99970)
	for i := 0; i < 19; i++ {
		e.BCC = append(e.BCC, fmt.Sprintf("hidden%d@example.test", i))
	}
	e.Attachments = []EmailAttachment{{"muestra.txt", "text/plain", make([]byte, MaxEmailAttachmentBytes)}}
	if err := ValidateEmail(e); err != nil {
		t.Fatal(err)
	}
	e = validTransportEmail()
	e.Recipient, e.To = e.To[0], nil
	if err := ValidateEmail(e); err != nil {
		t.Fatal("legacy test email rejected", err)
	}
}
