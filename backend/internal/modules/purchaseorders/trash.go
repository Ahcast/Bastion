package purchaseorders

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"strings"
	"time"
)

type TrashInput struct {
	Version uint64 `json:"version,string"`
	Reason  string `json:"reason"`
}

func (s *Service) Trash(ctx context.Context, a core.Actor, id, key string, p TrashInput, restore bool) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	p.Reason = strings.TrimSpace(p.Reason)
	if core.ID(id) != nil || p.Version == 0 || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "ordenes.delete"); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "ordenes.delete", true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "ordenes.view", o.CreatedBy, o.TeamID, false); e != nil {
			if e == core.Forbidden {
				return core.Missing
			}
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "ordenes.delete", o.CreatedBy, o.TeamID, true); e != nil {
			return e
		}
		op := "purchase_orders.delete:" + id
		if restore {
			op = "purchase_orders.restore:" + id
		}
		out, e = s.Core.Idempotent(ctx, tx, a, op, key, p, func() (any, error) {
			if o.Version != p.Version || (restore && o.Deleted == nil) || (!restore && o.Deleted != nil) {
				return nil, core.Conflict
			}
			if !restore {
				if e = s.items(ctx, tx, &o); e != nil {
					return nil, e
				}
				if !editableOrder(o) || o.SourceQuoteID != nil {
					return nil, core.Conflict
				}
				var count int
				for _, query := range []string{`SELECT COUNT(*) FROM customer_purchase_order_documents WHERE purchase_order_id=?`, `SELECT COUNT(*) FROM customer_purchase_order_status_history WHERE purchase_order_id=?`, `SELECT COUNT(*) FROM customer_purchase_order_clones WHERE source_purchase_order_id=?`} {
					if e = tx.QueryRowContext(ctx, query, id).Scan(&count); e != nil {
						return nil, e
					}
					if count > 0 {
						return nil, core.Conflict
					}
				}
			}
			value := "UTC_TIMESTAMP(6)"
			topic := "purchase_order.deleted"
			if restore {
				value = "NULL"
				topic = "purchase_order.restored"
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET deleted_at=`+value+`,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, topic, "customer_purchase_orders", id, "success", map[string]string{"version": fmt.Sprint(p.Version + 1), "reason": p.Reason}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, topic, "customer_purchase_orders", id, p.Version+1); e != nil {
				return nil, e
			}
			var deleted *time.Time
			if e = tx.QueryRowContext(ctx, `SELECT deleted_at FROM customer_purchase_orders WHERE id=?`, id).Scan(&deleted); e != nil {
				return nil, e
			}
			return map[string]any{"id": id, "version": fmt.Sprint(p.Version + 1), "deleted_at": deleted}, nil
		})
		return e
	})
	return out, orderDBError(e)
}
