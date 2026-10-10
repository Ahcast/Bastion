package quotes

import (
	"context"
	"database/sql"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type EmailAuthorizationView struct {
	ID            string    `json:"id"`
	QuoteVersion  uint64    `json:"quote_version,string"`
	PDFSnapshotID string    `json:"pdf_snapshot_id"`
	CreatedBy     string    `json:"created_by"`
	To            []string  `json:"to"`
	CC            []string  `json:"cc"`
	BCC           []string  `json:"bcc"`
	AttachmentIDs []string  `json:"attachment_ids"`
	Subject       string    `json:"subject"`
	Message       string    `json:"message"`
	ScheduledAt   time.Time `json:"scheduled_at"`
	ApprovalID    string    `json:"approval_id,omitempty"`
	MessageID     string    `json:"message_id,omitempty"`
	JobID         string    `json:"job_id,omitempty"`
	Status        string    `json:"status"`
	ErrorCode     string    `json:"error_code,omitempty"`
	AcceptedAt    *time.Time `json:"accepted_at,omitempty"`
	CreatedAt     time.Time `json:"created_at"`
	UpdatedAt     time.Time `json:"updated_at"`
}

type EmailAuthorizationPage struct {
	Data    []EmailAuthorizationView `json:"data"`
	Total   int                      `json:"total"`
	Page    int                      `json:"page"`
	PerPage int                      `json:"per_page"`
}

func (s *Service) EmailAuthorizations(ctx context.Context, a core.Actor, quoteID string, page, perPage int) (EmailAuthorizationPage, error) {
	out := EmailAuthorizationPage{Data: []EmailAuthorizationView{}, Page: page, PerPage: perPage}
	if page < 1 || perPage < 1 || perPage > 100 {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if _, e := s.access(ctx, tx, a, quoteID, "cotizaciones.view", false, false); e != nil {
			return e
		}
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM quote_email_intents WHERE quote_id=?`, quoteID).Scan(&out.Total); e != nil {
			return e
		}
		rows, e := tx.QueryContext(ctx, `SELECT CAST(id AS CHAR),quote_version,CAST(pdf_snapshot_id AS CHAR),CAST(created_by AS CHAR),to_json,cc_json,bcc_json,attachment_ids_json,subject,body,scheduled_at,COALESCE(CAST(approval_id AS CHAR),''),COALESCE(message_id,''),COALESCE(job_id,''),status,COALESCE(error_code,''),accepted_at,created_at,updated_at
			FROM quote_email_intents WHERE quote_id=? ORDER BY id DESC LIMIT ? OFFSET ?`, quoteID, perPage, (page-1)*perPage)
		if e != nil {
			return e
		}
		defer rows.Close()
		for rows.Next() {
			var v EmailAuthorizationView
			var toRaw, ccRaw, bccRaw, attachmentsRaw string
			var accepted sql.NullTime
			if e = rows.Scan(&v.ID, &v.QuoteVersion, &v.PDFSnapshotID, &v.CreatedBy, &toRaw, &ccRaw, &bccRaw, &attachmentsRaw, &v.Subject, &v.Message, &v.ScheduledAt, &v.ApprovalID, &v.MessageID, &v.JobID, &v.Status, &v.ErrorCode, &accepted, &v.CreatedAt, &v.UpdatedAt); e != nil {
				return e
			}
			if v.To, e = decodeStrings(toRaw); e != nil {
				return e
			}
			if v.CC, e = decodeStrings(ccRaw); e != nil {
				return e
			}
			if v.BCC, e = decodeStrings(bccRaw); e != nil {
				return e
			}
			if v.AttachmentIDs, e = decodeStrings(attachmentsRaw); e != nil {
				return e
			}
			if accepted.Valid {
				t := accepted.Time.UTC()
				v.AcceptedAt = &t
			}
			out.Data = append(out.Data, v)
		}
		return rows.Err()
	})
	return out, e
}
