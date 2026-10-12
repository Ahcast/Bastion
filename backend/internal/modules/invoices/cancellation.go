package invoices

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
)

type CancelInput struct {
	Version                          uint64 `json:"version,string"`
	Reason                           string `json:"reason"`
	OriginalAndCopyMarkedAndArchived bool   `json:"original_and_copy_marked_and_archived"`
}

type CancelResult struct {
	ID            string    `json:"id"`
	Version       uint64    `json:"version,string"`
	Status        string    `json:"status"`
	InvoiceNumber string    `json:"invoice_number"`
	CancelledAt   time.Time `json:"cancelled_at"`
}

func (s *Service) Cancel(ctx context.Context, a core.Actor, id, key string, p CancelInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || key == "" || p.Version == 0 || !finance.ValidText(p.Reason, 1000, true) || !p.OriginalAndCopyMarkedAndArchived {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gateSensitive(ctx, tx, a, "facturas.cancel", true); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "facturas.cancel", true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "facturas.view", o.CreatedBy, o.TeamID, false); e != nil {
			if e == core.Forbidden {
				return core.Missing
			}
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "invoices.cancel:"+id, key, p, func() (any, error) {
			if o.Version != p.Version || o.Status != "issued" || o.Number == nil || o.SourcePurchaseOrderID != nil || o.CancelledAt != nil || o.PaidTotal != "0.00" || o.Balance != o.Calculation.Total {
				return nil, core.Conflict
			}
			if _, e := s.checkedPaymentProjection(ctx, tx, o); e != nil {
				return nil, e
			}
			var payments, documents int
			if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM payments WHERE invoice_id=?`, id).Scan(&payments); e != nil {
				return nil, e
			}
			if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM invoice_document_snapshots WHERE invoice_id=?`, id).Scan(&documents); e != nil {
				return nil, e
			}
			if payments != 0 || documents != 0 {
				return nil, core.Conflict
			}
			now := time.Now().UTC().Truncate(time.Microsecond)
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE invoices SET status='cancelled',cancelled_at=?,cancelled_by=?,cancel_reason=?,cancellation_attested=TRUE,version=version+1,updated_by=?,updated_at=? WHERE id=? AND version=? AND status='issued'`, now, a.UserID, p.Reason, a.UserID, now, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "invoice.cancelled", "invoices", id, "success", map[string]string{"reason": p.Reason, "version": fmt.Sprint(p.Version + 1), "attested": "true"}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "invoice.cancelled", "invoices", id, p.Version+1); e != nil {
				return nil, e
			}
			return CancelResult{ID: id, Version: p.Version + 1, Status: "cancelled", InvoiceNumber: *o.Number, CancelledAt: now}, nil
		})
		if e != nil {
			return e
		}
		var result CancelResult
		raw, e := json.Marshal(out)
		if e != nil {
			return e
		}
		if e = json.Unmarshal(raw, &result); e != nil {
			return e
		}
		_, e = s.access(ctx, tx, a, result.ID, "facturas.view", true)
		return e
	})
	return out, dbError(e)
}
