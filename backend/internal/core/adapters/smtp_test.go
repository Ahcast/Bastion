package adapters

import (
	"bytes"
	"context"
	"encoding/base64"
	"io"
	"mime"
	"mime/multipart"
	"mime/quotedprintable"
	"net/mail"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func TestSMTPMIMEEnvelopeAndAttachments(t *testing.T) {
	e := core.Email{To: []string{"one@example.test", "two@example.test"}, CC: []string{"copy@example.test"}, BCC: []string{"hidden@example.test"}, Subject: "Cotización de instalación eléctrica", Body: "Estimado cliente:\nAdjuntamos la cotización pública.\nSaludos.", MessageID: "<0123456789abcdef0123456789abcdef@bastion.castilloit.net>", Attachments: []core.EmailAttachment{{Filename: "cotización.pdf", MediaType: "application/pdf", Data: []byte("%PDF-1.4\nsynthetic approved bytes\x00\xff")}, {Filename: "evidencia.txt", MediaType: "text/plain", Data: []byte("Evidencia sintética: información autorizada.")}}}
	m, err := smtpMessage(core.SMTPSettings{From: "sender@example.test"}, e)
	if err != nil {
		t.Fatal(err)
	}
	recipients, err := m.GetRecipients()
	if err != nil {
		t.Fatal(err)
	}
	for i, recipient := range recipients {
		address, err := mail.ParseAddress(recipient)
		if err != nil {
			t.Fatal(err)
		}
		recipients[i] = address.Address
	}
	sort.Strings(recipients)
	want := []string{"copy@example.test", "hidden@example.test", "one@example.test", "two@example.test"}
	if !reflect.DeepEqual(recipients, want) {
		t.Fatal("SMTP envelope recipients changed", recipients)
	}
	var wire bytes.Buffer
	if _, err = m.WriteTo(&wire); err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(wire.Bytes(), []byte("hidden@example.test")) {
		t.Fatal("BCC leaked into MIME")
	}
	parsed, err := mail.ReadMessage(bytes.NewReader(wire.Bytes()))
	if err != nil {
		t.Fatal(err)
	}
	if parsed.Header.Get("Bcc") != "" || parsed.Header.Get("Message-ID") != e.MessageID {
		t.Fatal("private recipient leaked or message identity changed")
	}
	subject, err := (&mime.WordDecoder{}).DecodeHeader(parsed.Header.Get("Subject"))
	if err != nil || subject != e.Subject {
		t.Fatal("Unicode subject changed", err)
	}
	for header, count := range map[string]int{"To": 2, "Cc": 1} {
		list, err := parsed.Header.AddressList(header)
		if err != nil || len(list) != count {
			t.Fatal("visible recipients changed", header, err)
		}
	}
	media, params, err := mime.ParseMediaType(parsed.Header.Get("Content-Type"))
	if err != nil || media != "multipart/mixed" {
		t.Fatal("MIME attachment container missing", err)
	}
	reader := multipart.NewReader(parsed.Body, params["boundary"])
	partIndex := 0
	for {
		part, err := reader.NextPart()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatal(err)
		}
		var content io.Reader = part
		switch strings.ToLower(part.Header.Get("Content-Transfer-Encoding")) {
		case "base64":
			content = base64.NewDecoder(base64.StdEncoding, part)
		case "quoted-printable":
			content = quotedprintable.NewReader(part)
		}
		data, err := io.ReadAll(content)
		if err != nil {
			t.Fatal(err)
		}
		if partIndex == 0 {
			if strings.ReplaceAll(string(data), "\r\n", "\n") != e.Body {
				t.Fatal("public body changed")
			}
		} else {
			if partIndex > len(e.Attachments) {
				t.Fatal("unexpected attachment")
			}
			file := e.Attachments[partIndex-1]
			filename, err := (&mime.WordDecoder{}).DecodeHeader(part.FileName())
			if err != nil || filename != file.Filename || !bytes.Equal(data, file.Data) {
				t.Fatalf("attachment name or bytes changed: filename=%q decoded=%q lengths=%d/%d err=%v", part.FileName(), filename, len(data), len(file.Data), err)
			}
			mt, _, err := mime.ParseMediaType(part.Header.Get("Content-Type"))
			if err != nil || mt != file.MediaType {
				t.Fatal("attachment type changed", err)
			}
		}
		partIndex++
	}
	if partIndex != 3 {
		t.Fatal("missing MIME parts", partIndex)
	}
}
func TestSMTPRejectsInvalidBeforeConnecting(t *testing.T) {
	e := core.Email{Recipient: "customer@example.test", Subject: "Quote\r\nBcc: leak@example.test", MessageID: "<0123456789abcdef0123456789abcdef@bastion.castilloit.net>"}
	if err := (SMTP{}).Send(context.Background(), core.SMTPSettings{Host: "unreachable.invalid", Port: 587, TLS: "starttls"}, "synthetic", e); err != core.Invalid {
		t.Fatal("invalid message reached network", err)
	}
}
