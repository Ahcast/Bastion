package quotes

import (
	"context"
	"database/sql"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

// QueueEmail consumes the exact commercial authorization and durably schedules
// its delivery. SMTP is deliberately performed later by the worker.
func (s *Service) QueueEmail(ctx context.Context, a core.Actor, quoteID, intentID, key string) (any, error) {
	if s.Approvals == nil || a.Type != "user" {
		if s.Approvals == nil {
			return nil, core.Disabled
		}
		return nil, core.Forbidden
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if !s.Core.IntegrationEnabledUsing(ctx, tx, "email") || !s.Core.IntegrationEnabledUsing(ctx, tx, "files") {
			return core.Disabled
		}
		q, e := s.access(ctx, tx, a, quoteID, "cotizaciones.send", true, true)
		if e != nil {
			return e
		}
		intent, e := s.loadEmailIntent(ctx, tx, intentID, quoteID)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.email_queue:"+intentID, key, map[string]string{"intent_id": intent.ID, "binding_hash": intent.BindingHash}, func() (any, error) {
			if intent.Status != "approved" || intent.ApprovalID == "" || intent.QuoteVersion != q.Version {
				return nil, core.Conflict
			}
			binding, e := s.emailBinding(ctx, tx, a, q, intent)
			if e != nil || binding.Hash != intent.BindingHash {
				if e != nil {
					return nil, e
				}
				return nil, core.Conflict
			}
			approval, e := s.Approvals.Get(ctx, tx, intent.ApprovalID, "quote_email_intents", intent.ID)
			if e != nil {
				return nil, e
			}
			// Consumption and durable enqueue share this transaction. A crash cannot
			// consume approval without preserving the send intention, or vice versa.
			if e := s.Approvals.Consume(ctx, tx, a, approval.ApprovalID, binding, "cotizaciones.send"); e != nil {
				return nil, e
			}
			messageID := "<" + security.ID() + "@bastion.castilloit.net>"
			jobID, e := s.Core.EnqueueAt(ctx, tx, a, "quote.email.send", "cotizaciones.send", map[string]string{"quote_id": quoteID, "intent_id": intent.ID}, intent.ScheduledAt)
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quote_email_intents SET status='queued',message_id=?,job_id=?,error_code=NULL,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status='approved'`, messageID, jobID, intent.ID)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.email_queued", "quotes", quoteID, "queued", map[string]string{"intent_id": intent.ID, "approval_id": intent.ApprovalID, "job_id": jobID, "message_id": messageID}); e != nil {
				return nil, e
			}
			return map[string]string{"id": intent.ID, "approval_id": intent.ApprovalID, "job_id": jobID, "message_id": messageID, "status": "queued"}, nil
		})
		return e
	})
	return out, e
}
