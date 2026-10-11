package purchaseorders

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type TransitionInput struct {
	ToStatus string `json:"to_status"`
	Version  uint64 `json:"version,string"`
}

type StatusHistoryEntry struct {
	ID           string    `json:"id"`
	ItemPosition *int      `json:"item_position"`
	FromStatus   string    `json:"from_status"`
	ToStatus     string    `json:"to_status"`
	ActorID      string    `json:"actor_id"`
	Version      uint64    `json:"purchase_order_version,string"`
	CreatedAt    time.Time `json:"created_at"`
}

type StatusHistoryPage struct {
	Data    []StatusHistoryEntry `json:"data"`
	Total   int64                `json:"total"`
	Page    int                  `json:"page"`
	PerPage int                  `json:"per_page"`
}

func editableOrder(o PurchaseOrder) bool {
	if o.Status != StatusReceived {
		return false
	}
	for _, item := range o.Items {
		if item.Status != ItemStatusPending {
			return false
		}
	}
	return true
}

// Transition changes only status. Position is 1-based; zero denotes the order.
// The aggregate lock serializes item changes, header changes and full saves.
func (s *Service) Transition(ctx context.Context, a core.Actor, id, key string, position int, p TransitionInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version == 0 || position < 0 || position > 200 {
		return nil, core.Invalid
	}
	permission := "ordenes.transition"
	if position != 0 {
		permission = "ordenes.items.transition"
		if !IsItemStatus(p.ToStatus) {
			return nil, core.Invalid
		}
	} else {
		if !IsOrderStatus(p.ToStatus) {
			return nil, core.Invalid
		}
		if p.ToStatus == StatusDelivered {
			permission = "ordenes.deliver"
		}
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, permission); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, permission, true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "ordenes.view", o.CreatedBy, o.TeamID, false); e != nil {
			if e == core.Forbidden {
				return core.Missing
			}
			return e
		}
		// Replays also require current permission and recent identity for delivery.
		if position == 0 && p.ToStatus == StatusDelivered {
			if e = s.Core.Authorize(ctx, tx, a, permission, o.CreatedBy, o.TeamID, true); e != nil {
				return e
			}
		}
		op := fmt.Sprintf("purchase_orders.transition:%s:%d", id, position)
		out, e = s.Core.Idempotent(ctx, tx, a, op, key, p, func() (any, error) {
			if o.Version != p.Version {
				return nil, core.Conflict
			}
			if e := s.items(ctx, tx, &o); e != nil {
				return nil, e
			}
			from := o.Status
			topic := "purchase_order.status_changed"
			var historyPosition any
			if position != 0 {
				if position > len(o.Items) {
					return nil, core.Missing
				}
				from = o.Items[position-1].Status
				if !CanItemTransition(from, p.ToStatus) || o.Status == StatusDelivered || o.Status == StatusInvoiced || o.Status == StatusClosed {
					return nil, core.Conflict
				}
				if e := core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_order_items SET status=?,updated_at=UTC_TIMESTAMP(6) WHERE purchase_order_id=? AND position=? AND status=?`, p.ToStatus, id, position-1, from)); e != nil {
					return nil, e
				}
				historyPosition = position
				topic = "purchase_order.item_status_changed"
			} else {
				if !CanOrderTransition(from, p.ToStatus) {
					return nil, core.Conflict
				}
				if p.ToStatus == StatusDelivered {
					if len(o.Items) == 0 {
						return nil, core.Conflict
					}
					for _, item := range o.Items {
						if item.Status != ItemStatusDelivered {
							return nil, core.Conflict
						}
					}
				}
				o.Status = p.ToStatus
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET status=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, o.Status, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			o.Version++
			if _, e := tx.ExecContext(ctx, `INSERT INTO customer_purchase_order_status_history(purchase_order_id,item_position,from_status,to_status,actor_id,purchase_order_version,created_at) VALUES(?,?,?,?,?,?,UTC_TIMESTAMP(6))`, id, historyPosition, from, p.ToStatus, a.UserID, o.Version); e != nil {
				return nil, e
			}
			details := map[string]any{"version": fmt.Sprint(o.Version), "from_status": from, "to_status": p.ToStatus, "item_position": historyPosition}
			if e := s.Core.Audit(ctx, tx, a, topic, "customer_purchase_orders", id, "success", details); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, topic, "customer_purchase_orders", id, o.Version); e != nil {
				return nil, e
			}
			if position == 0 && p.ToStatus == StatusDelivered {
				if e := s.Core.EmitEvent(ctx, tx, a, "purchase_order.delivered", "customer_purchase_orders", id, o.Version); e != nil {
					return nil, e
				}
			}
			return map[string]string{"id": id, "version": fmt.Sprint(o.Version), "status": o.Status, "to_status": p.ToStatus}, nil
		})
		return e
	})
	return out, orderDBError(e)
}

func (s *Service) StatusHistory(ctx context.Context, a core.Actor, id string, page, perPage int) (StatusHistoryPage, error) {
	out := StatusHistoryPage{Data: []StatusHistoryEntry{}, Page: page, PerPage: perPage}
	if page < 1 || page > 100000 || perPage < 1 || perPage > 100 {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "ordenes.view"); e != nil {
			return e
		}
		// Lock header so COUNT and rows describe the same history window.
		if _, e := s.access(ctx, tx, a, id, "ordenes.view", true); e != nil {
			return e
		}
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM customer_purchase_order_status_history WHERE purchase_order_id=?`, id).Scan(&out.Total); e != nil {
			return e
		}
		rows, e := tx.QueryContext(ctx, `SELECT CAST(id AS CHAR),item_position,from_status,to_status,CAST(actor_id AS CHAR),purchase_order_version,created_at FROM customer_purchase_order_status_history WHERE purchase_order_id=? ORDER BY id DESC LIMIT ? OFFSET ?`, id, perPage, (page-1)*perPage)
		if e != nil {
			return e
		}
		defer rows.Close()
		for rows.Next() {
			var entry StatusHistoryEntry
			var position sql.NullInt64
			if e = rows.Scan(&entry.ID, &position, &entry.FromStatus, &entry.ToStatus, &entry.ActorID, &entry.Version, &entry.CreatedAt); e != nil {
				return e
			}
			if position.Valid {
				v := int(position.Int64)
				entry.ItemPosition = &v
			}
			out.Data = append(out.Data, entry)
		}
		return rows.Err()
	})
	return out, e
}
