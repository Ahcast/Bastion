package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
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
	e := tx.QueryRowContext(ctx, `SELECT v.object_key,v.filename,v.media_type,v.size_bytes,v.checksum
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
		e = tx.QueryRowContext(ctx, `SELECT v.object_key,v.filename,v.media_type,v.size_bytes,v.checksum
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

// jobQuoteAuthorizes a durable user identity from its current grants. Jobs must
// never depend on the browser session that happened to exist when they were queued.
func (s *Service) jobQuote(ctx context.Context, tx *sql.Tx, user, id, permission string) (Quote, error) {
	if core.ID(user) != nil || core.ID(id) != nil {
		return Quote{}, core.Invalid
	}
	q, e := scan(tx.QueryRowContext(ctx, `SELECT `+columns+` FROM quotes WHERE id=? FOR UPDATE`, id))
	if e == sql.ErrNoRows {
		return q, core.Missing
	}
	if e != nil {
		return q, e
	}
	if e = s.Core.AuthorizeGrantHolder(ctx, tx, user, permission, q.Owner, q.Team); e != nil {
		if e == core.Forbidden {
			return q, core.Missing
		}
		return q, e
	}
	return q, nil
}

func (s *Service) jobDocumentGrant(ctx context.Context, tx *sql.Tx, user, document, quote string) error {
	var owner, team, entity, entityID string
	var deleted sql.NullTime
	e := tx.QueryRowContext(ctx, `SELECT CAST(owner_id AS CHAR),COALESCE(CAST(team_id AS CHAR),''),COALESCE(entity_type,''),COALESCE(entity_id,''),deleted_at FROM documents WHERE id=?`, document).Scan(&owner, &team, &entity, &entityID, &deleted)
	if e == sql.ErrNoRows || deleted.Valid {
		return core.Missing
	}
	if e != nil {
		return e
	}
	if entity != "quotes" || entityID != quote {
		return core.Conflict
	}
	if e = s.Core.AuthorizeGrantHolder(ctx, tx, user, "core.documents.read", owner, team); e != nil {
		return core.Missing
	}
	return nil
}

func (s *Service) jobEmailEvidence(ctx context.Context, tx *sql.Tx, user string, q Quote, intent quoteEmailIntent) (map[string]any, error) {
	var pdfDocument, pdfHash, pdfBinding string
	var pdfSize int64
	e := tx.QueryRowContext(ctx, `SELECT CAST(p.document_id AS CHAR),p.input_hash,COALESCE(p.binding_hash,''),v.size_bytes
		FROM quote_pdf_snapshots p
		JOIN documents d ON d.id=p.document_id
		JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version
		WHERE p.id=? AND p.quote_id=? AND p.quote_version=? AND p.kind='issued' AND p.status='available' AND d.deleted_at IS NULL AND v.status='available'`, intent.PDFSnapshotID, q.ID, q.Version).Scan(&pdfDocument, &pdfHash, &pdfBinding, &pdfSize)
	if e == sql.ErrNoRows {
		return nil, core.Conflict
	}
	if e != nil {
		return nil, e
	}
	if e = s.jobDocumentGrant(ctx, tx, user, pdfDocument, q.ID); e != nil {
		return nil, e
	}
	total := pdfSize
	attachments := make([]map[string]any, 0, len(intent.AttachmentIDs))
	for _, id := range intent.AttachmentIDs {
		if e = s.jobDocumentGrant(ctx, tx, user, id, q.ID); e != nil {
			return nil, e
		}
		var generated bool
		var number int
		var filename, media, checksum, status string
		var size int64
		e = tx.QueryRowContext(ctx, `SELECT d.is_generated,d.current_version,v.filename,v.media_type,v.size_bytes,v.checksum,v.status
			FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version WHERE d.id=?`, id).Scan(&generated, &number, &filename, &media, &size, &checksum, &status)
		if e != nil {
			return nil, e
		}
		if generated || status != "available" {
			return nil, core.Conflict
		}
		total += size
		if total > core.MaxEmailAttachmentBytes {
			return nil, core.Invalid
		}
		attachments = append(attachments, map[string]any{"document_id": id, "version": number, "filename": filename, "media_type": media, "size_bytes": size, "checksum": checksum})
	}
	return map[string]any{"pdf_snapshot_id": intent.PDFSnapshotID, "pdf_document_id": pdfDocument, "pdf_input_hash": pdfHash, "pdf_binding_hash": pdfBinding, "pdf_size_bytes": pdfSize, "attachments": attachments}, nil
}

func (s *Service) jobEmailBindingHash(ctx context.Context, tx *sql.Tx, user string, q Quote, intent quoteEmailIntent) (string, error) {
	if q.Deleted != nil || q.Status != "approved" || q.Version != intent.QuoteVersion {
		return "", core.Conflict
	}
	if e := s.Core.AuthorizeGrantHolder(ctx, tx, user, "cotizaciones.view", q.Owner, q.Team); e != nil {
		return "", e
	}
	evidence, e := s.jobEmailEvidence(ctx, tx, user, q, intent)
	if e != nil {
		return "", e
	}
	input := struct {
		Schema       int            `json:"schema"`
		QuoteID      string         `json:"quote_id"`
		QuoteVersion uint64         `json:"quote_version,string"`
		To           []string       `json:"to"`
		CC           []string       `json:"cc"`
		BCC          []string       `json:"bcc"`
		Subject      string         `json:"subject"`
		Message      string         `json:"message"`
		ScheduledAt  string         `json:"scheduled_at"`
		Evidence     map[string]any `json:"evidence"`
	}{quoteEmailBindingSchema, q.ID, q.Version, intent.To, intent.CC, intent.BCC, intent.Subject, intent.Message, intent.ScheduledAt.UTC().Format(time.RFC3339Nano), evidence}
	raw, e := json.Marshal(input)
	if e != nil {
		return "", e
	}
	hash := security.Hash(string(raw))
	if s.Approvals == nil {
		return "", core.Disabled
	}
	v, e := s.Approvals.Get(ctx, tx, intent.ApprovalID, "quote_email_intents", intent.ID)
	if e != nil {
		return "", e
	}
	b := workflow.Binding{Action: "quote.send", Entity: "quote_email_intents", ID: intent.ID, Version: 1, Hash: hash, Owner: q.Owner, Team: q.Team, ViewPermission: "cotizaciones.view", RequestPermission: "cotizaciones.request_approval", DecisionPermission: "cotizaciones.approve", RevokePermission: "cotizaciones.revoke_approval"}
	if e = s.Approvals.VerifyConsumed(ctx, tx, v, b); e != nil {
		return "", e
	}
	return hash, nil
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

// Transient database/network failures preserve the resumable preparation state.
func (s *Service) rejectEmailClientError(ctx context.Context, id, code string, cause error) {
	var ce *core.Error
	if errors.As(cause, &ce) && ce.Status >= 400 && ce.Status < 500 && ce.Status != 429 {
		_ = s.failEmailIntent(ctx, id, code)
	}
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

// AuthorizeEmailJob applies current record scopes without a browser session.
func (s *Service) AuthorizeEmailJob(ctx context.Context, j core.Job) error {
	if j.Kind != "quote.email.send" || j.Permission != "cotizaciones.send" {
		return core.Forbidden
	}
	var payload struct {
		QuoteID string `json:"quote_id"`
	}
	if json.Unmarshal(j.Payload, &payload) != nil {
		return core.Invalid
	}
	return s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.jobQuote(ctx, tx, j.UserID, payload.QuoteID, "cotizaciones.send")
		if e != nil {
			return e
		}
		return s.Core.AuthorizeGrantHolder(ctx, tx, j.UserID, "cotizaciones.view", q.Owner, q.Team)
	})
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
			q, e := s.jobQuote(ctx, tx, j.UserID, payload.QuoteID, "cotizaciones.view")
			if e != nil {
				return e
			}
			hash, e := s.jobEmailBindingHash(ctx, tx, j.UserID, q, intent)
			if e != nil || hash != intent.BindingHash {
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
			s.rejectEmailClientError(ctx, payload.IntentID, "preparation_rejected", e)
			return e
		}

		email := core.Email{ID: intent.ID, To: append([]string{}, intent.To...), CC: append([]string{}, intent.CC...), BCC: append([]string{}, intent.BCC...), Subject: intent.Subject, Body: intent.Message, MessageID: intent.MessageID}
		for _, f := range files {
			b, e := reader.ReadObject(ctx, f.Key, f.Size)
			if e != nil {
				return e
			}
			if int64(len(b)) != f.Size || security.Hash(string(b)) != f.Checksum {
				_ = s.failEmailIntent(ctx, intent.ID, "attachment_integrity_failed")
				return core.Conflict
			}
			email.Attachments = append(email.Attachments, core.EmailAttachment{Filename: f.Filename, MediaType: f.MediaType, Data: b})
		}
		if e = core.ValidateEmail(email); e != nil {
			_ = s.failEmailIntent(ctx, intent.ID, "envelope_invalid")
			return e
		}
		if e = s.AuthorizeEmailJob(ctx, j); e != nil {
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
			q, e := s.jobQuote(ctx, tx, j.UserID, payload.QuoteID, "cotizaciones.view")
			if e != nil {
				return e
			}
			current, e := s.loadEmailIntent(ctx, tx, intent.ID, payload.QuoteID)
			if e != nil {
				return e
			}
			current.Status = "queued"
			hash, e := s.jobEmailBindingHash(ctx, tx, j.UserID, q, current)
			if e != nil || hash != intent.BindingHash {
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
			s.rejectEmailClientError(ctx, intent.ID, "pre_send_revalidation_failed", e)
			return e
		}
		if e = s.AuthorizeEmailJob(ctx, j); e != nil {
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
			// Record the historical send even if a user revised the quote during SMTP.
			q, e := scan(tx.QueryRowContext(ctx, `SELECT `+columns+` FROM quotes WHERE id=? FOR UPDATE`, payload.QuoteID))
			if e != nil {
				return e
			}
			if q.Status == "approved" && q.Version == intent.QuoteVersion && q.Deleted == nil {
				if e = s.items(ctx, tx, &q); e != nil {
					return e
				}
				if e = core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET status='sent',version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, a.UserID, q.ID, q.Version)); e != nil {
					return e
				}
				q.Status = "sent"
				q.Version++
				if _, e = s.record(ctx, tx, a, q, "quote.sent"); e != nil {
					return e
				}
			}
			return s.Core.Audit(ctx, tx, a, "quote.email_accepted", "quotes", payload.QuoteID, "accepted", map[string]string{"intent_id": intent.ID, "job_id": j.ID, "message_id": intent.MessageID})
		})
	}
}
