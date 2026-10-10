package core

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"time"
)

type Email struct {
	ID, Recipient, Subject, Body, MessageID string
	To, CC, BCC                             []string
	Attachments                             []EmailAttachment
}
type EmailAttachment struct {
	Filename, MediaType string
	Data                []byte
}
type Mailer interface {
	Send(context.Context, SMTPSettings, string, Email) error
}

func (s *Service) QueueTestEmail(ctx context.Context, a Actor, key string) (any, error) {
	if !s.IntegrationEnabled(ctx, "email") {
		return nil, Disabled
	}
	u, e := s.Profile(ctx, a.UserID)
	if e != nil {
		return nil, e
	}
	var result any
	e = s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.email.test", "", "", true); e != nil {
			return e
		}
		var err error
		result, err = s.Idempotent(ctx, tx, a, "email.test", key, map[string]string{"recipient": u.Email}, func() (any, error) {
			id := security.ID()
			messageID := "<" + id + "@bastion.castilloit.net>"
			if _, e := tx.ExecContext(ctx, "INSERT INTO email_messages VALUES(?,?,?,'Prueba SMTP de Bastion','La configuración SMTP de Bastion ha sido verificada.','pending',?,NULL,UTC_TIMESTAMP(6))", id, a.UserID, u.Email, messageID); e != nil {
				return nil, e
			}
			job, e := s.Enqueue(ctx, tx, a, "email.test", "core.email.test", map[string]string{"email_id": id})
			if e != nil {
				return nil, e
			}
			if e = s.Audit(ctx, tx, a, "email.queue_test", "email", id, "pending", map[string]string{}); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "job_id": job, "status": "pending"}, nil
		})
		return err
	})
	return result, e
}
func (s *Service) EmailHandler(mailer Mailer) JobHandler {
	return func(ctx context.Context, j Job) error {
		if !s.IntegrationEnabled(ctx, "email") || mailer == nil {
			return Disabled
		}
		var payload struct {
			EmailID string `json:"email_id"`
		}
		if json.Unmarshal(j.Payload, &payload) != nil {
			return Invalid
		}
		var email Email
		var settings SMTPSettings
		var password string
		var done bool
		e := s.Transact(ctx, func(tx *sql.Tx) error {
			var lease string
			if e := tx.QueryRowContext(ctx, "SELECT lease_token FROM jobs WHERE id=? AND lease_until>UTC_TIMESTAMP(6) FOR UPDATE", j.ID).Scan(&lease); e != nil || lease != j.Lease {
				return Conflict
			}
			var status string
			if e := tx.QueryRowContext(ctx, "SELECT id,recipient,subject,body,message_id,status FROM email_messages WHERE id=? FOR UPDATE", payload.EmailID).Scan(&email.ID, &email.Recipient, &email.Subject, &email.Body, &email.MessageID, &status); e != nil {
				return e
			}
			if status == "accepted" {
				done = true
				return nil
			}
			if status != "pending" {
				return ErrUncertain
			}
			var raw string
			var encrypted sql.NullString
			if e := tx.QueryRowContext(ctx, "SELECT value,secret FROM settings WHERE name='smtp'").Scan(&raw, &encrypted); e != nil {
				return e
			}
			if json.Unmarshal([]byte(raw), &settings) != nil || !contains(s.Config.SMTPHosts, settings.Host) {
				return Invalid
			}
			var e error
			password, e = security.Decrypt(s.Config.Key, encrypted.String)
			if e != nil {
				return e
			}
			_, e = tx.ExecContext(ctx, "UPDATE email_messages SET status='sending',updated_at=UTC_TIMESTAMP(6) WHERE id=?", email.ID)
			return e
		})
		if e != nil || done {
			return e
		}
		if e = s.JobAllowed(ctx, j); e != nil {
			return s.emailUncertain(ctx, email.ID)
		}
		sendCtx, cancel := context.WithTimeout(ctx, 20*time.Second)
		defer cancel()
		if e = mailer.Send(sendCtx, settings, password, email); e != nil {
			return s.emailUncertain(ctx, email.ID)
		}
		return Changed(s.DB.ExecContext(ctx, "UPDATE email_messages SET status='accepted',updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='sending'", email.ID))
	}
}
func (s *Service) emailUncertain(ctx context.Context, id string) error {
	_, e := s.DB.ExecContext(ctx, "UPDATE email_messages SET status='uncertain',error_code='external_result_uncertain',updated_at=UTC_TIMESTAMP(6) WHERE id=?", id)
	if e != nil {
		return e
	}
	return ErrUncertain
}
func (s *Service) Emails(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.email.read", "", "", false); e != nil {
		return nil, e
	}
	return s.Rows(ctx, "SELECT id,recipient,subject,status,message_id,error_code,updated_at FROM email_messages ORDER BY updated_at DESC LIMIT 100")
}
