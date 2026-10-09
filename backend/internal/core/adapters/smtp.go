package adapters

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	mail "github.com/wneessen/go-mail"
	"time"
)

type SMTP struct{}

func (SMTP) Send(ctx context.Context, c core.SMTPSettings, password string, e core.Email) error {
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
	m := mail.NewMsg()
	if err = m.From(c.From); err != nil {
		return err
	}
	if err = m.To(e.Recipient); err != nil {
		return err
	}
	m.Subject(e.Subject)
	m.SetBodyString(mail.TypeTextPlain, e.Body)
	m.SetGenHeader(mail.HeaderMessageID, e.MessageID)
	return client.DialAndSendWithContext(ctx, m)
}
