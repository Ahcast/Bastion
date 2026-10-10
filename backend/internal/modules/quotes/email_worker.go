package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

type EmailObjectReader interface {
	ReadObject(context.Context, string, int64) ([]byte, error)
}

type emailFile struct {
	Key, Filename, MediaType, Checksum string
	Size                               int64
}

func (s *Service) emailFiles(ctx context.Context, tx *sql.Tx, intent quoteEmailIntent) ([]emailFile, error) {
	files := make([]emailFile, 0, 1+len(intent.AttachmentIDs))
	var issued emailFile
	e := tx.QueryRowContext(ctx, `SELECT v.storage_key,v.filename,v.media_type,v.size_bytes,v.checksum
		FROM quote_pdf_snapshots p
		JOIN documents d ON d.id=p.document_id
		JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version
		WHERE p.id=? AND p.quote_id=? AND p.quote_version=? AND p.kind='issued' AND p.status='available' AND d.deleted_at IS NULL AND v.status='available'`, intent.PDFSnapshotID, intent.QuoteID, intent.QuoteVersion).Scan(&issued.Key, &issued.Filename, &issued.MediaType, &issued.Size, &issued.Checksum)
	if e == sql.ErrNoRows {
		return nil, core.Conflict
	}
	if e != nil {
		return nil, e
	}
	files = append(files, issued)
	for _, id := range intent.AttachmentIDs {
		var f emailFile
		e = tx.QueryRowContext(ctx, `SELECT v.storage_key,v.filename,v.media_type,v.size_bytes,v.checksum
			FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version
			WHERE d.id=? AND d.entity_type='quotes' AND d.entity_id=? AND d.is_generated=FALSE AND d.deleted_at IS NULL AND v.status='available'`, id, intent.QuoteID).Scan(&f.Key, &f.Filename, &f.MediaType, &f.Size, &f.Checksum)
		if e == sql.ErrNoRows {
			return nil, core.Conflict
		}
		if e != nil {
			return nil, e
		}
		files = append(files, f)
	}
	return files, nil
}

func smtpHostAllowed(host string, allowed []string) bool {
	for _, v := range allowed {
		if strings.EqualFold(strings.TrimSpace(v), host) {
			return true
		}
	}
	return false
}

func (s *Service) failEmailIntent(ctx context.Context, id, code string) error {
	_, e := s.Core.DB.ExecContext(ctx, `UPDATE quote_email_intents SET status='failed',error_code=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status IN ('queued','preparing')`, code, id)
	return e
}

func (s *Service) failSendingBeforeExternal(ctx context.Context, id, code string) error {
	_, e := s.Core.DB.ExecContext(ctx, `UPDATE quote_email_intents SET status='failed',error_code=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='sending'`, code, id)
	return e
}

func (s *Service) uncertainEmailIntent(ctx context.Context, id string) error {
	_, e := s.Core.DB.ExecContext(ctx, `UPDATE quote_email_intents SET status='uncertain',error_code='external_result_uncertain',updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='sending'`, id)
	if e != nil {
		return e
	}
	return core.ErrUncertain
}

// CommercialEmailHandler revalidates the approved binding both before and
// after private object reads. Only the final SMTP call can create an uncertain
// outcome; all earlier failures remain safely retryable or rejectable.
func (s *Service) CommercialEmailHandler(reader EmailObjectReader, mailer core.Mailer) core.JobHandler {
	return func(ctx context.Context, j core.Job) error {
		if reader == nil || mailer == nil || !s.Core.IntegrationEnabled(ctx, "email") || !s.Core.IntegrationEnabled(ctx, "files") {
			return core.Disabled
		}
		var payload struct {
			QuoteID  string `json:"quote_id"`
			IntentID string `json:"intent_id"`
		}
		if json.Unmarshal(j.Payload, &payload) != nil || core.ID(payload.QuoteID) != nil || core.ID(payload.IntentID) != nil {
			return core.Invalid
		}
		a := core.Actor{Type: "user", ID: j.UserID, UserID: j.UserID, Source: "job", RequestID: security.ID(), CorrelationID: j.CorrelationID}
		var intent quoteEmailIntent
		var files []emailFile
		alreadyAccepted := false
		e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
			var lease string
			if e := tx.QueryRowContext(ctx, `SELECT lease_token FROM jobs WHERE id=? AND lease_until>UTC_TIMESTAMP(6) FOR UPDATE`, j.ID).Scan(&lease); e != nil || lease != j.Lease {
				return core.Conflict
			}
			var e error
			intent, e = s.loadEmailIntent(ctx, tx, payload.IntentID, payload.QuoteID)
			if e != nil {
				return e
			}
			if intent.JobID != j.ID {
				return core.Conflict
			}
			switch intent.Status {
			case "accepted":
				alreadyAccepted = true
				return nil
			case "sending", "uncertain":
				return core.ErrUncertain
			case "queued", "preparing":
			default:
				return core.Conflict
			}
			q, e := s.access(ctx, tx, a, payload.QuoteID, "cotizaciones.view", false, false)
			if e != nil {
				return e
			}
			binding, e := s.emailBinding(ctx, tx, a, q, intent)
			if e != nil || binding.Hash != intent.BindingHash {
				if e != nil {
					return e
				}
				return core.Conflict
			}
			files, e = s.emailFiles(ctx, tx, intent)
			if e != nil {
				return e
			}
			if intent.Status == "preparing" {
				return nil
			}
			return core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET status='preparing',error_code=NULL,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='queued'`, intent.ID))
		})
		if alreadyAccepted {
			return nil
		}
		if e != nil {
			if e == core.ErrUncertain {
				if intent.Status == "sending" {
					return s.uncertainEmailIntent(ctx, payload.IntentID)
				}
				return core.ErrUncertain
			}
			_ = s.failEmailIntent(ctx, payload.IntentID, "preparation_rejected")
			return e
		}

		email := core.Email{ID: intent.ID, To: append([]string{}, intent.To...), CC: append([]string{}, intent.CC...), BCC: append([]string{}, intent.BCC...), Subject: intent.Subject, Body: intent.Message, MessageID: intent.MessageID}
		for _, f := range files {
			b, e := reader.ReadObject(ctx, f.Key, f.Size)
			if e != nil || int64(len(b)) != f.Size || security.Hash(string(b)) != f.Checksum {
				_ = s.failEmailIntent(ctx, intent.ID, "attachment_integrity_failed")
				if e != nil {
					return e
				}
				return core.Conflict
			}
			email.Attachments = append(email.Attachments, core.EmailAttachment{Filename: f.Filename, MediaType: f.MediaType, Data: b})
		}
		if e = core.ValidateEmail(email); e != nil {
			_ = s.failEmailIntent(ctx, intent.ID, "envelope_invalid")
			return e
		}
		if e = s.Core.JobAllowed(ctx, j); e != nil {
			_ = s.failEmailIntent(ctx, intent.ID, "authorization_revoked")
			return e
		}

		var settings core.SMTPSettings
		var password string
		e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
			var lease, status, bindingHash string
			if e := tx.QueryRowContext(ctx, `SELECT j.lease_token,i.status,i.binding_hash FROM jobs j JOIN quote_email_intents i ON i.job_id=j.id WHERE j.id=? AND i.id=? AND j.lease_until>UTC_TIMESTAMP(6) FOR UPDATE`, j.ID, intent.ID).Scan(&lease, &status, &bindingHash); e != nil || lease != j.Lease || status != "preparing" || bindingHash != intent.BindingHash {
				return core.Conflict
			}
			q, e := s.access(ctx, tx, a, payload.QuoteID, "cotizaciones.view", false, false)
			if e != nil {
				return e
			}
			current, e := s.loadEmailIntent(ctx, tx, intent.ID, payload.QuoteID)
			if e != nil {
				return e
			}
			current.Status = "queued"
			binding, e := s.emailBinding(ctx, tx, a, q, current)
			if e != nil || binding.Hash != intent.BindingHash {
				if e != nil {
					return e
				}
				return core.Conflict
			}
			var raw string
			var encrypted sql.NullString
			if e = tx.QueryRowContext(ctx, `SELECT value,secret FROM settings WHERE name='smtp'`).Scan(&raw, &encrypted); e != nil {
				return e
			}
			if json.Unmarshal([]byte(raw), &settings) != nil || !smtpHostAllowed(settings.Host, s.Core.Config.SMTPHosts) {
				return core.Invalid
			}
			password, e = security.Decrypt(s.Core.Config.Key, encrypted.String)
			if e != nil {
				return e
			}
			return core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET status='sending',updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='preparing'`, intent.ID))
		})
		if e != nil {
			_ = s.failEmailIntent(ctx, intent.ID, "pre_send_revalidation_failed")
			return e
		}
		if e = s.Core.JobAllowed(ctx, j); e != nil {
			// The network call has not started yet, so this state is known failed,
			// not uncertain, even though the sending marker was already persisted.
			_ = s.failSendingBeforeExternal(ctx, intent.ID, "authorization_revoked")
			return e
		}
		sendCtx, cancel := context.WithTimeout(ctx, 20*time.Second)
		defer cancel()
		if e = mailer.Send(sendCtx, settings, password, email); e != nil {
			return s.uncertainEmailIntent(ctx, intent.ID)
		}
		return s.Core.Transact(ctx, func(tx *sql.Tx) error {
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET status='accepted',accepted_at=UTC_TIMESTAMP(6),error_code=NULL,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='sending'`, intent.ID)); e != nil {
				return e
			}
			return s.Core.Audit(ctx, tx, a, "quote.email_accepted", "quotes", payload.QuoteID, "accepted", map[string]string{"intent_id": intent.ID, "job_id": j.ID, "message_id": intent.MessageID})
		})
	}
}
