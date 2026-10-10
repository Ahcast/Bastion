package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

const quoteEmailBindingSchema = 1

type EmailAuthorizationRequest struct {
	QuoteVersion   uint64   `json:"quote_version,string"`
	PDFSnapshotID  string   `json:"pdf_snapshot_id"`
	To             []string `json:"to"`
	CC             []string `json:"cc"`
	BCC            []string `json:"bcc"`
	AttachmentIDs  []string `json:"attachment_ids"`
	Subject        string   `json:"subject"`
	Message        string   `json:"message"`
	ScheduledAt    string   `json:"scheduled_at"`
	ExpiresAt      string   `json:"expires_at"`
	Reason         string   `json:"reason"`
}

type EmailAuthorizationDecision struct {
	Version  uint64 `json:"version,string"`
	Decision string `json:"decision"`
	Reason   string `json:"reason"`
}

type EmailAuthorizationRevoke struct {
	Version uint64 `json:"version,string"`
	Reason  string `json:"reason"`
}

type quoteEmailIntent struct {
	ID             string
	QuoteID        string
	QuoteVersion   uint64
	PDFSnapshotID  string
	CreatedBy      string
	To             []string
	CC             []string
	BCC            []string
	AttachmentIDs  []string
	Subject        string
	Message        string
	ScheduledAt    time.Time
	BindingHash    string
	ApprovalID     string
	Status         string
	MessageID      string
	JobID          string
	ErrorCode      string
}

func normalizeQuoteEmailRequest(p *EmailAuthorizationRequest) (time.Time, time.Time, error) {
	p.Reason = strings.TrimSpace(p.Reason)
	if p.QuoteVersion == 0 || core.ID(p.PDFSnapshotID) != nil || !workflow.ReasonValid(p.Reason, false) {
		return time.Time{}, time.Time{}, core.Invalid
	}
	if len(p.AttachmentIDs) > 9 {
		return time.Time{}, time.Time{}, core.Invalid
	}
	seen := map[string]bool{}
	for _, id := range p.AttachmentIDs {
		if core.ID(id) != nil || seen[id] {
			return time.Time{}, time.Time{}, core.Invalid
		}
		seen[id] = true
	}
	sort.Strings(p.AttachmentIDs)
	probe := core.Email{To: append([]string{}, p.To...), CC: append([]string{}, p.CC...), BCC: append([]string{}, p.BCC...), Subject: p.Subject, Body: p.Message, MessageID: "<00000000000000000000000000000000@bastion.castilloit.net>"}
	if e := core.ValidateEmail(probe); e != nil {
		return time.Time{}, time.Time{}, e
	}
	scheduled, e := time.Parse(time.RFC3339Nano, p.ScheduledAt)
	if e != nil || scheduled.Year() < 1000 || scheduled.Year() > 9999 {
		return time.Time{}, time.Time{}, core.Invalid
	}
	expires, e := time.Parse(time.RFC3339Nano, p.ExpiresAt)
	if e != nil || expires.Year() < 1000 || expires.Year() > 9999 || !expires.After(time.Now()) {
		return time.Time{}, time.Time{}, core.Invalid
	}
	scheduled = scheduled.UTC().Truncate(time.Microsecond)
	expires = expires.UTC().Truncate(time.Microsecond)
	p.ScheduledAt = scheduled.Format(time.RFC3339Nano)
	p.ExpiresAt = expires.Format(time.RFC3339Nano)
	return scheduled, expires, nil
}

func encodeStrings(v []string) (string, error) {
	if v == nil {
		v = []string{}
	}
	b, e := json.Marshal(v)
	return string(b), e
}

func decodeStrings(raw string) ([]string, error) {
	var out []string
	if e := json.Unmarshal([]byte(raw), &out); e != nil || out == nil {
		return nil, core.Invalid
	}
	return out, nil
}

func (s *Service) emailEvidence(ctx context.Context, tx *sql.Tx, a core.Actor, q Quote, pdfSnapshot string, attachments []string) (map[string]any, error) {
	var pdfDocument, pdfHash, pdfBinding string
	var pdfSize int64
	e := tx.QueryRowContext(ctx, `SELECT CAST(p.document_id AS CHAR),p.input_hash,COALESCE(p.binding_hash,''),v.size_bytes
		FROM quote_pdf_snapshots p
		JOIN documents d ON d.id=p.document_id
		JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version
		WHERE p.id=? AND p.quote_id=? AND p.quote_version=? AND p.kind='issued' AND p.status='available' AND d.deleted_at IS NULL AND v.status='available'
		FOR UPDATE`, pdfSnapshot, q.ID, q.Version).Scan(&pdfDocument, &pdfHash, &pdfBinding, &pdfSize)
	if e == sql.ErrNoRows {
		return nil, core.Conflict
	}
	if e != nil {
		return nil, e
	}
	if _, _, e = s.Core.DocumentAccess(ctx, tx, a, pdfDocument, "core.documents.read"); e != nil {
		return nil, e
	}
	total := pdfSize
	files := make([]map[string]any, 0, len(attachments))
	for _, id := range attachments {
		if _, _, e = s.Core.DocumentAccess(ctx, tx, a, id, "core.documents.read"); e != nil {
			return nil, e
		}
		var entity, entityID, filename, media, checksum, status string
		var number int
		var size int64
		var generated bool
		e = tx.QueryRowContext(ctx, `SELECT COALESCE(d.entity_type,''),COALESCE(d.entity_id,''),d.is_generated,d.current_version,v.filename,v.media_type,v.size_bytes,v.checksum,v.status
			FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version
			WHERE d.id=? AND d.deleted_at IS NULL FOR UPDATE`, id).Scan(&entity, &entityID, &generated, &number, &filename, &media, &size, &checksum, &status)
		if e != nil {
			return nil, core.Missing
		}
		if entity != "quotes" || entityID != q.ID || generated || status != "available" {
			return nil, core.Conflict
		}
		total += size
		if total > core.MaxEmailAttachmentBytes {
			return nil, core.Invalid
		}
		files = append(files, map[string]any{"document_id": id, "version": number, "filename": filename, "media_type": media, "size_bytes": size, "checksum": checksum})
	}
	return map[string]any{"pdf_snapshot_id": pdfSnapshot, "pdf_document_id": pdfDocument, "pdf_input_hash": pdfHash, "pdf_binding_hash": pdfBinding, "pdf_size_bytes": pdfSize, "attachments": files}, nil
}

func (s *Service) emailBinding(ctx context.Context, tx *sql.Tx, a core.Actor, q Quote, intent quoteEmailIntent) (workflow.Binding, error) {
	var b workflow.Binding
	if q.Deleted != nil || q.Status != "approved" || q.Version != intent.QuoteVersion {
		return b, core.Conflict
	}
	evidence, e := s.emailEvidence(ctx, tx, a, q, intent.PDFSnapshotID, intent.AttachmentIDs)
	if e != nil {
		return b, e
	}
	input := struct {
		Schema      int            `json:"schema"`
		QuoteID     string         `json:"quote_id"`
		QuoteVersion uint64        `json:"quote_version,string"`
		To          []string       `json:"to"`
		CC          []string       `json:"cc"`
		BCC         []string       `json:"bcc"`
		Subject     string         `json:"subject"`
		Message     string         `json:"message"`
		ScheduledAt string         `json:"scheduled_at"`
		Evidence    map[string]any `json:"evidence"`
	}{quoteEmailBindingSchema, q.ID, q.Version, intent.To, intent.CC, intent.BCC, intent.Subject, intent.Message, intent.ScheduledAt.UTC().Format(time.RFC3339Nano), evidence}
	raw, e := json.Marshal(input)
	if e != nil {
		return b, e
	}
	if len(raw) > 1024*1024 {
		return b, core.Invalid
	}
	return workflow.Binding{Action: "quote.send", Entity: "quote_email_intents", ID: intent.ID, Version: 1, Hash: security.Hash(string(raw)), Owner: q.Owner, Team: q.Team, ViewPermission: "cotizaciones.view", RequestPermission: "cotizaciones.request_approval", DecisionPermission: "cotizaciones.approve", RevokePermission: "cotizaciones.revoke_approval"}, nil
}

func (s *Service) loadEmailIntent(ctx context.Context, tx *sql.Tx, id, quote string) (quoteEmailIntent, error) {
	var v quoteEmailIntent
	var toRaw, ccRaw, bccRaw, attachmentsRaw string
	var approval, messageID, jobID, errorCode sql.NullString
	e := tx.QueryRowContext(ctx, `SELECT CAST(id AS CHAR),CAST(quote_id AS CHAR),quote_version,CAST(pdf_snapshot_id AS CHAR),CAST(created_by AS CHAR),to_json,cc_json,bcc_json,attachment_ids_json,subject,body,scheduled_at,binding_hash,approval_id,message_id,job_id,status,error_code
		FROM quote_email_intents WHERE id=? AND quote_id=? FOR UPDATE`, id, quote).Scan(&v.ID, &v.QuoteID, &v.QuoteVersion, &v.PDFSnapshotID, &v.CreatedBy, &toRaw, &ccRaw, &bccRaw, &attachmentsRaw, &v.Subject, &v.Message, &v.ScheduledAt, &v.BindingHash, &approval, &messageID, &jobID, &v.Status, &errorCode)
	if e == sql.ErrNoRows {
		return v, core.Missing
	}
	if e != nil {
		return v, e
	}
	if v.To, e = decodeStrings(toRaw); e != nil {
		return v, e
	}
	if v.CC, e = decodeStrings(ccRaw); e != nil {
		return v, e
	}
	if v.BCC, e = decodeStrings(bccRaw); e != nil {
		return v, e
	}
	if v.AttachmentIDs, e = decodeStrings(attachmentsRaw); e != nil {
		return v, e
	}
	v.ApprovalID, v.MessageID, v.JobID, v.ErrorCode = approval.String, messageID.String, jobID.String, errorCode.String
	return v, nil
}

func (s *Service) RequestEmailAuthorization(ctx context.Context, a core.Actor, id, key string, p EmailAuthorizationRequest) (any, error) {
	if s.Approvals == nil || a.Type != "user" {
		if s.Approvals == nil {
			return nil, core.Disabled
		}
		return nil, core.Forbidden
	}
	scheduled, expires, e := normalizeQuoteEmailRequest(&p)
	if e != nil {
		return nil, e
	}
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.access(ctx, tx, a, id, "cotizaciones.send", true, false)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.request_approval", q.Owner, q.Team, false); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.email_authorization:"+id, key, p, func() (any, error) {
			if q.Deleted != nil || q.Status != "approved" || q.Version != p.QuoteVersion {
				return nil, core.Conflict
			}
			toRaw, e := encodeStrings(p.To)
			if e != nil {
				return nil, e
			}
			ccRaw, e := encodeStrings(p.CC)
			if e != nil {
				return nil, e
			}
			bccRaw, e := encodeStrings(p.BCC)
			if e != nil {
				return nil, e
			}
			attachmentsRaw, e := encodeStrings(p.AttachmentIDs)
			if e != nil {
				return nil, e
			}
			r, e := tx.ExecContext(ctx, `INSERT INTO quote_email_intents(quote_id,quote_version,pdf_snapshot_id,created_by,to_json,cc_json,bcc_json,attachment_ids_json,subject,body,scheduled_at,binding_hash,status,created_at,updated_at)
				VALUES(?,?,?,?,?,?,?,?,?,?,?,'','pending_approval',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, q.ID, q.Version, p.PDFSnapshotID, a.UserID, toRaw, ccRaw, bccRaw, attachmentsRaw, p.Subject, p.Message, scheduled)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			intent := quoteEmailIntent{ID: core.WrapID(n), QuoteID: q.ID, QuoteVersion: q.Version, PDFSnapshotID: p.PDFSnapshotID, CreatedBy: a.UserID, To: p.To, CC: p.CC, BCC: p.BCC, AttachmentIDs: p.AttachmentIDs, Subject: p.Subject, Message: p.Message, ScheduledAt: scheduled, Status: "pending_approval"}
			binding, e := s.emailBinding(ctx, tx, a, q, intent)
			if e != nil {
				return nil, e
			}
			approval, e := s.Approvals.Request(ctx, tx, a, binding, expires, p.Reason)
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET binding_hash=?,approval_id=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='pending_approval'`, binding.Hash, approval.ApprovalID, intent.ID)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.email_authorization_requested", "quotes", q.ID, "pending", map[string]string{"intent_id": intent.ID, "approval_id": approval.ApprovalID, "quote_version": fmt.Sprint(q.Version)}); e != nil {
				return nil, e
			}
			return map[string]string{"id": intent.ID, "approval_id": approval.ApprovalID, "status": "pending_approval", "quote_version": fmt.Sprint(q.Version)}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) DecideEmailAuthorization(ctx context.Context, a core.Actor, quoteID, intentID, key string, p EmailAuthorizationDecision) (any, error) {
	if s.Approvals == nil || a.Type != "user" {
		if s.Approvals == nil {
			return nil, core.Disabled
		}
		return nil, core.Forbidden
	}
	p.Reason = strings.TrimSpace(p.Reason)
	if p.Version == 0 || (p.Decision != "approved" && p.Decision != "rejected") || !workflow.ReasonValid(p.Reason, true) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.access(ctx, tx, a, quoteID, "cotizaciones.approve", true, true)
		if e != nil {
			return e
		}
		intent, e := s.loadEmailIntent(ctx, tx, intentID, quoteID)
		if e != nil {
			return e
		}
		if intent.ApprovalID == "" || intent.Status != "pending_approval" || intent.QuoteVersion != q.Version {
			return core.Conflict
		}
		approval, e := s.Approvals.Get(ctx, tx, intent.ApprovalID, "quote_email_intents", intent.ID)
		if e != nil {
			return e
		}
		if a.UserID == approval.RequestedBy || a.UserID == q.Owner {
			return core.Forbidden
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.email_authorization_decide:"+intentID, key, p, func() (any, error) {
			binding, e := s.emailBinding(ctx, tx, a, q, intent)
			if e != nil || binding.Hash != intent.BindingHash {
				if e != nil {
					return nil, e
				}
				return nil, core.Conflict
			}
			if _, e = s.Approvals.Decide(ctx, tx, a, approval, binding, p.Version, p.Decision, p.Reason); e != nil {
				return nil, e
			}
			status := "approved"
			if p.Decision == "rejected" {
				status = "rejected"
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET status=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='pending_approval'`, status, intent.ID)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.email_authorization_"+p.Decision, "quotes", quoteID, "success", map[string]string{"intent_id": intent.ID, "approval_id": intent.ApprovalID}); e != nil {
				return nil, e
			}
			return map[string]string{"id": intent.ID, "approval_id": intent.ApprovalID, "status": status}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) RevokeEmailAuthorization(ctx context.Context, a core.Actor, quoteID, intentID, key string, p EmailAuthorizationRevoke) (any, error) {
	if s.Approvals == nil || a.Type != "user" {
		if s.Approvals == nil {
			return nil, core.Disabled
		}
		return nil, core.Forbidden
	}
	p.Reason = strings.TrimSpace(p.Reason)
	if p.Version == 0 || !workflow.ReasonValid(p.Reason, true) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.access(ctx, tx, a, quoteID, "cotizaciones.revoke_approval", true, true)
		if e != nil {
			return e
		}
		intent, e := s.loadEmailIntent(ctx, tx, intentID, quoteID)
		if e != nil {
			return e
		}
		if intent.ApprovalID == "" || (intent.Status != "pending_approval" && intent.Status != "approved") {
			return core.Conflict
		}
		approval, e := s.Approvals.Get(ctx, tx, intent.ApprovalID, "quote_email_intents", intent.ID)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.email_authorization_revoke:"+intentID, key, p, func() (any, error) {
			binding, e := s.emailBinding(ctx, tx, a, q, intent)
			if e != nil || binding.Hash != intent.BindingHash {
				if e != nil {
					return nil, e
				}
				return nil, core.Conflict
			}
			if _, e = s.Approvals.Revoke(ctx, tx, a, approval, p.Version, p.Reason); e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET status='revoked',updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status IN ('pending_approval','approved')`, intent.ID)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.email_authorization_revoked", "quotes", quoteID, "success", map[string]string{"intent_id": intent.ID, "approval_id": intent.ApprovalID}); e != nil {
				return nil, e
			}
			return map[string]string{"id": intent.ID, "approval_id": intent.ApprovalID, "status": "revoked"}, nil
		})
		return e
	})
	return out, e
}
