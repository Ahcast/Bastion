package purchaseorders

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
)

type InvoiceConversionInput struct {
	Version     uint64  `json:"version,string"`
	InvoiceDate string  `json:"invoice_date"`
	PaymentTerm string  `json:"payment_term_type"`
	CreditDays  *uint16 `json:"credit_days"`
	PublicNotes string  `json:"public_notes"`
	Reason      string  `json:"reason"`
}
type InvoiceClosureInput struct {
	Version        uint64 `json:"version,string"`
	InvoiceVersion uint64 `json:"invoice_version,string"`
	Reason         string `json:"reason"`
}
type LinkedInvoice struct {
	ID                    string  `json:"id"`
	Version               uint64  `json:"version,string"`
	Number                *string `json:"invoice_number"`
	Status                string  `json:"status"`
	Currency              string  `json:"currency"`
	Total                 string  `json:"total"`
	PaidTotal             string  `json:"paid_total"`
	Balance               string  `json:"balance"`
	SourcePurchaseOrderID *string `json:"source_purchase_order_id"`
}
type InvoiceTarget interface {
	AuthorizePurchaseOrderInvoice(context.Context, *sql.Tx, core.Actor, string, *string) error
	CreateFromPurchaseOrder(context.Context, *sql.Tx, core.Actor, PurchaseOrder, InvoiceConversionInput) (string, error)
	ReadForPurchaseOrder(context.Context, *sql.Tx, core.Actor, string) (LinkedInvoice, error)
}

func (s *Service) billingAccess(ctx context.Context, tx *sql.Tx, a core.Actor, id, permission string, sensitive bool) (PurchaseOrder, error) {
	if a.Type != "user" {
		return PurchaseOrder{}, core.Forbidden
	}
	if e := s.gate(ctx, tx, a, permission); e != nil {
		return PurchaseOrder{}, e
	}
	o, e := s.access(ctx, tx, a, id, permission, true)
	if e != nil {
		return o, e
	}
	if e = s.Core.Authorize(ctx, tx, a, "ordenes.view", o.CreatedBy, o.TeamID, false); e != nil {
		return o, e
	}
	if sensitive {
		if e = s.Core.Authorize(ctx, tx, a, permission, o.CreatedBy, o.TeamID, true); e != nil {
			return o, e
		}
	}
	return o, nil
}
func deliveredForInvoice(o PurchaseOrder) bool {
	if o.Deleted != nil || o.Status != StatusDelivered || len(o.Items) == 0 {
		return false
	}
	for _, x := range o.Items {
		if x.Status != ItemStatusDelivered {
			return false
		}
	}
	return true
}
func (s *Service) ConvertToInvoice(ctx context.Context, a core.Actor, id, key string, p InvoiceConversionInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	if s.Invoices == nil {
		return nil, core.Disabled
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		o, e := s.billingAccess(ctx, tx, a, id, "ordenes.convert", false)
		if e != nil {
			return e
		}
		if e = s.Invoices.AuthorizePurchaseOrderInvoice(ctx, tx, a, o.TeamID, o.InvoiceID); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "purchase_orders.convert_to_invoice:"+id, key, p, func() (any, error) {
			if o.Version != p.Version || o.InvoiceID != nil {
				return nil, core.Conflict
			}
			if e := s.items(ctx, tx, &o); e != nil {
				return nil, e
			}
			if !deliveredForInvoice(o) {
				return nil, core.Conflict
			}
			invoiceID, e := s.Invoices.CreateFromPurchaseOrder(ctx, tx, a, o, p)
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET invoice_id=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND status='delivered' AND invoice_id IS NULL`, invoiceID, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "purchase_order.invoice_linked", "customer_purchase_orders", id, "success", map[string]string{"version": fmt.Sprint(o.Version + 1), "source_version": fmt.Sprint(o.Version), "invoice_id": invoiceID, "reason": p.Reason}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "purchase_order.invoice_linked", "customer_purchase_orders", id, o.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(o.Version + 1), "status": o.Status, "invoice_id": invoiceID, "invoice_version": "1"}, nil
		})
		return e
	})
	return out, orderDBError(e)
}

// LockInvoiceSource is used by the authorized Invoice owner, before its row lock.
func (s *Service) LockInvoiceSource(ctx context.Context, tx *sql.Tx, a core.Actor, id, invoiceID string) (PurchaseOrder, error) {
	o, e := s.billingAccess(ctx, tx, a, id, "ordenes.convert", false)
	if e != nil {
		return o, e
	}
	if o.InvoiceID == nil || *o.InvoiceID != invoiceID {
		return o, core.Conflict
	}
	if e = s.items(ctx, tx, &o); e != nil {
		return o, e
	}
	return o, nil
}

// LockInvoiceLedger checks a source invariant only; it grants no OC read/write
// capability. The Invoice owner has already authorized the human and invoice.
func (s *Service) LockInvoiceLedger(ctx context.Context, tx *sql.Tx, a core.Actor, id, invoiceID string, mutable bool) (bool, error) {
	if a.Type != "user" {
		return false, core.Forbidden
	}
	var status string
	var linked *string
	var deleted sql.NullTime
	e := tx.QueryRowContext(ctx, `SELECT status,CAST(invoice_id AS CHAR),deleted_at FROM customer_purchase_orders WHERE id=? FOR UPDATE`, id).Scan(&status, &linked, &deleted)
	if e == sql.ErrNoRows {
		return false, core.Missing
	}
	if e != nil {
		return false, e
	}
	if deleted.Valid || linked == nil || *linked != invoiceID || mutable && status == StatusClosed {
		return status == StatusClosed, core.Conflict
	}
	return status == StatusClosed, nil
}
func (s *Service) MarkInvoiceIssued(ctx context.Context, tx *sql.Tx, a core.Actor, id, invoiceID, number string, version uint64) error {
	o, e := s.billingAccess(ctx, tx, a, id, "ordenes.convert", true)
	if e != nil {
		return e
	}
	if e = s.items(ctx, tx, &o); e != nil {
		return e
	}
	if !deliveredForInvoice(o) || o.InvoiceID == nil || *o.InvoiceID != invoiceID || o.Version != version || version == ^uint64(0) || !finance.ValidText(number, 80, true) {
		return core.Conflict
	}
	if e = core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET status='invoiced',invoice_number_snapshot=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND invoice_id=? AND status='delivered'`, number, a.UserID, id, version, invoiceID)); e != nil {
		return e
	}
	return s.billingHistory(ctx, tx, a, o, StatusInvoiced, "purchase_order.invoiced", "Factura emitida "+number)
}
func (s *Service) billingHistory(ctx context.Context, tx *sql.Tx, a core.Actor, o PurchaseOrder, to, topic, reason string) error {
	if _, e := tx.ExecContext(ctx, `INSERT INTO customer_purchase_order_status_history(purchase_order_id,item_position,from_status,to_status,actor_id,purchase_order_version,created_at) VALUES(?,NULL,?,?,?,?,UTC_TIMESTAMP(6))`, o.ID, o.Status, to, a.UserID, o.Version+1); e != nil {
		return e
	}
	if e := s.Core.Audit(ctx, tx, a, topic, "customer_purchase_orders", o.ID, "success", map[string]string{"version": fmt.Sprint(o.Version + 1), "from_status": o.Status, "to_status": to, "reason": reason}); e != nil {
		return e
	}
	return s.Core.EmitEvent(ctx, tx, a, topic, "customer_purchase_orders", o.ID, o.Version+1)
}
func (s *Service) CloseInvoice(ctx context.Context, a core.Actor, id, key string, p InvoiceClosureInput, reopen bool) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) || p.InvoiceVersion == 0 || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	if s.Invoices == nil {
		return nil, core.Disabled
	}
	permission, from, to, topic := "ordenes.close", StatusInvoiced, StatusClosed, "purchase_order.closed"
	if reopen {
		permission, from, to, topic = "ordenes.reopen", StatusClosed, StatusInvoiced, "purchase_order.reopened"
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		o, e := s.billingAccess(ctx, tx, a, id, permission, true)
		if e != nil {
			return e
		}
		if o.InvoiceID == nil {
			return core.Conflict
		}
		linked, e := s.Invoices.ReadForPurchaseOrder(ctx, tx, a, *o.InvoiceID)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, topic+":"+id, key, p, func() (any, error) {
			if o.Deleted != nil || o.Status != from || o.Version != p.Version || linked.Version != p.InvoiceVersion || linked.Status != "issued" || linked.SourcePurchaseOrderID == nil || *linked.SourcePurchaseOrderID != o.ID || linked.Currency != o.Currency || linked.Total != o.Calculation.Total || o.InvoiceNumber == nil || linked.Number == nil || *o.InvoiceNumber != *linked.Number || !reopen && linked.Balance != "0.00" {
				return nil, core.Conflict
			}
			if e := s.items(ctx, tx, &o); e != nil {
				return nil, e
			}
			if len(o.Items) == 0 {
				return nil, core.Conflict
			}
			for _, x := range o.Items {
				if x.Status != ItemStatusDelivered {
					return nil, core.Conflict
				}
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET status=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND status=?`, to, a.UserID, id, o.Version, from)); e != nil {
				return nil, e
			}
			if e = s.billingHistory(ctx, tx, a, o, to, topic, p.Reason); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(o.Version + 1), "status": to, "invoice_id": linked.ID, "invoice_version": fmt.Sprint(linked.Version)}, nil
		})
		return e
	})
	return out, orderDBError(e)
}
