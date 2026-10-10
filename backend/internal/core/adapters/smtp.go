package adapters

import (
	"bytes"
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	mail "github.com/wneessen/go-mail"
	"time"
)

type SMTP struct{}

func (SMTP) Send(ctx context.Context, c core.SMTPSettings, password string, e core.Email) error {
	m, err := smtpMessage(c, e)
	if err != nil {
		return err
	}
	opts := []mail.Option{mail.WithPort(c.Port), mail.WithTimeout(15 * time.Second), mail.WithUsername(c.Username), mail.WithPassword(password), mail.WithSMTPAuth(mail.SMTPAuthPlain)}
	if c.TLS == "implicit" {
		opts = append(opts, mail.WithSSL())
	} else if c.TLS == "starttls" {
		opts = append(opts, mail.WithTLSPolicy(mail.TLSMandatory))
	} else {
		return core.Invalid
	}
	client, err := mail.NewClient(c.Host, opts...)
	if err != nil {
		return err
	}
	return client.DialAndSendWithContext(ctx, m)
}

// smtpMessage is shared with MIME tests; production sends exactly these bytes.
func smtpMessage(c core.SMTPSettings, e core.Email) (*mail.Msg, error) {
	if err := core.ValidateEmail(e); err != nil {
		return nil, err
	}
	m := mail.NewMsg()
	var err error
	if err = m.From(c.From); err != nil {
		return nil, err
	}
	to := e.To
	if e.Recipient != "" {
		to = []string{e.Recipient}
	}
	if err = m.To(to...); err != nil {
		return nil, err
	}
	if len(e.CC) > 0 {
		if err = m.Cc(e.CC...); err != nil {
			return nil, err
		}
	}
	if len(e.BCC) > 0 {
		if err = m.Bcc(e.BCC...); err != nil {
			return nil, err
		}
	}
	m.Subject(e.Subject)
	m.SetBodyString(mail.TypeTextPlain, e.Body)
	m.SetGenHeader(mail.HeaderMessageID, e.MessageID)
	for _, file := range e.Attachments {
		if err = m.AttachReader(file.Filename, bytes.NewReader(file.Data), mail.WithFileContentType(mail.ContentType(file.MediaType))); err != nil {
			return nil, err
		}
	}
	return m, nil
}
