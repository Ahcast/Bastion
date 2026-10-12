package quotes

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"net/http"
	"strings"
)

type InvoiceConversionInput struct {
	Version            uint64  `json:"version,string"`
	InvoiceDate        string  `json:"invoice_date"`
	PaymentTerm        string  `json:"payment_term_type"`
	CreditDays         *uint16 `json:"credit_days"`
	PublicNotes        string  `json:"public_notes"`
	AdditionalExpenses string  `json:"additional_expenses"`
	Reason             string  `json:"reason"`
}
type InvoiceTarget interface {
	AuthorizeQuoteInvoice(context.Context, *sql.Tx, core.Actor, string, *string) error
	CreateFromQuote(context.Context, *sql.Tx, core.Actor, Quote, InvoiceConversionInput) (string, error)
}

func (s *Service) ConvertToInvoice(ctx context.Context, a core.Actor, id, key string, p InvoiceConversionInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) || strings.TrimSpace(key) == "" || !validText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	if s.Invoices == nil {
		return nil, core.Missing
	}
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
		if e = s.Invoices.AuthorizeQuoteInvoice(ctx, tx, a, q.Team, q.InvoiceID); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.convert_to_invoice:"+id, key, p, func() (any, error) {
			if q.Deleted != nil || q.Status != "accepted" || q.Version != p.Version || q.InvoiceID != nil || q.PurchaseOrderID != nil {
				return nil, core.Conflict
			}
			if e := s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			invoiceID, e := s.Invoices.CreateFromQuote(ctx, tx, a, q, p)
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET status='converted',invoice_id=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND status='accepted' AND invoice_id IS NULL AND purchase_order_id IS NULL`, invoiceID, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			q.Status = "converted"
			q.Version++
			q.InvoiceID = &invoiceID
			if _, e = s.record(ctx, tx, a, q, "quote.converted"); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.invoice_linked", "quotes", id, "success", map[string]string{"source_version": fmt.Sprint(p.Version), "invoice_id": invoiceID, "reason": p.Reason}); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(q.Version), "status": q.Status, "invoice_id": invoiceID, "invoice_version": "1"}, nil
		})
		return e
	})
	return out, e
}
func (s *Service) registerInvoiceConversion(register func(string, api.Action)) {
	if s.Invoices == nil {
		return
	}
	register("POST /api/v1/quotes/{id}/convert-to-invoice", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		var p InvoiceConversionInput
		if e := required(r, &p, "version", "invoice_date", "payment_term_type", "credit_days", "public_notes", "additional_expenses", "reason"); e != nil {
			return nil, e
		}
		return s.ConvertToInvoice(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
	})
}
