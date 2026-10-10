package quotes

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

// RevokeEmailAuthorizationSafe intentionally does not rebuild the approved
// content binding. Revocation must remain possible even when evidence has
// drifted; drift blocks execution, it must never block cancellation.
func (s *Service) RevokeEmailAuthorizationSafe(ctx context.Context, a core.Actor, quoteID, intentID, key string, p EmailAuthorizationRevoke) (any, error) {
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
		if _, e := s.access(ctx, tx, a, quoteID, "cotizaciones.revoke_approval", true, true); e != nil {
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
