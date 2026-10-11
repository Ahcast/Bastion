package quotes

import (
	"context"
	"database/sql"
	"fmt"
	"net/http"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
)

type PurchaseOrderConversionInput struct {
	Version              uint64  `json:"version,string"`
	OrderNumber          string  `json:"order_number"`
	Destination          string  `json:"destination"`
	OrderDate            string  `json:"order_date"`
	RequiredDeliveryDate *string `json:"required_delivery_date"`
	Comments             string  `json:"comments"`
	AdditionalExpenses   string  `json:"additional_expenses"`
}

// The target owner validates and writes its aggregate; neither module writes
// the other's private tables. Composition supplies the same transaction.
type PurchaseOrderTarget interface {
	AuthorizeQuoteConversion(context.Context, *sql.Tx, core.Actor, string, *string) error
	CreateFromQuote(context.Context, *sql.Tx, core.Actor, Quote, PurchaseOrderConversionInput) (string, error)
}

func (s *Service) ConvertToPurchaseOrder(ctx context.Context, a core.Actor, id, key string, p PurchaseOrderConversionInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version == 0 || p.Version >= 18446744073709551615 || strings.TrimSpace(key) == "" {
		return nil, core.Invalid
	}
	if s.PurchaseOrders == nil {
		return nil, core.Missing
	}
	p.OrderNumber = strings.TrimSpace(p.OrderNumber)
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.convert"); e != nil {
			return e
		}
		q, e := s.access(ctx, tx, a, id, "cotizaciones.convert", true, false)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
			return e
		}
		if e = s.PurchaseOrders.AuthorizeQuoteConversion(ctx, tx, a, q.Team, q.PurchaseOrderID); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.convert_to_purchase_order:"+id, key, p, func() (any, error) {
			if q.Deleted != nil || q.Status != "accepted" || q.Version != p.Version || q.PurchaseOrderID != nil {
				return nil, core.Conflict
			}
			if e := s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			orderID, e := s.PurchaseOrders.CreateFromQuote(ctx, tx, a, q, p)
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET status='converted',purchase_order_id=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND purchase_order_id IS NULL`, orderID, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			q.Status = "converted"
			q.Version++
			q.PurchaseOrderID = &orderID
			if _, e = s.record(ctx, tx, a, q, "quote.converted"); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.purchase_order_linked", "quotes", id, "success", map[string]string{"source_version": fmt.Sprint(p.Version), "purchase_order_id": orderID}); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(q.Version), "status": q.Status, "purchase_order_id": orderID, "purchase_order_version": "1"}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) registerPurchaseOrderConversion(register func(string, api.Action)) {
	if s.PurchaseOrders == nil {
		return
	}
	register("POST /api/v1/quotes/{id}/convert-to-purchase-order", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		var p PurchaseOrderConversionInput
		// Nullable delivery date must still be explicitly present.
		if e := required(r, &p, "version", "order_number", "destination", "order_date", "comments", "additional_expenses", "required_delivery_date"); e != nil {
			return nil, e
		}
		return s.ConvertToPurchaseOrder(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
	})
}

func nullableConversionField(v any, name string) bool {
	_, ok := v.(*PurchaseOrderConversionInput)
	return ok && name == "required_delivery_date"
}
