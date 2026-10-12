package invoices

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
)

type CloneInput struct {
	Version     uint64 `json:"version,string"`
	InvoiceDate string `json:"invoice_date"`
	PublicNotes string `json:"public_notes"`
}

func (s *Service) Clone(ctx context.Context, a core.Actor, id, key string, p CloneInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || key == "" || p.Version == 0 || !fiscalDate(p.InvoiceDate) || !finance.ValidText(p.PublicNotes, 12000, false) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "facturas.create"); e != nil {
			return e
		}
		source, e := s.access(ctx, tx, a, id, "facturas.view", true)
		if e != nil {
			return e
		}
		if e = s.destination(ctx, tx, a, "facturas.create", a.UserID, source.TeamID); e != nil {
			return e
		}
		if e = s.items(ctx, tx, &source); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "invoices.clone:"+id, key, p, func() (any, error) {
			if source.Version != p.Version || source.Status != "draft" && source.Status != "issued" || source.CancelledAt != nil {
				return nil, core.Conflict
			}
			old, e := Validate(persistedInput(source))
			if e != nil || old.Calculation != source.Calculation || len(old.Items) != len(source.Items) {
				return nil, core.Conflict
			}
			for i, x := range old.Items {
				if x != source.Items[i].CalculatedItem {
					return nil, core.Conflict
				}
			}
			input := persistedInput(source)
			input.InvoiceDate = p.InvoiceDate
			input.PublicNotes = p.PublicNotes
			input.Version = 0
			for i := range input.Items {
				input.Items[i].ID = ""
			}
			calculated, e := Validate(input)
			if e != nil {
				return nil, e
			}
			if calculated.Calculation != source.Calculation {
				return nil, core.Conflict
			}
			dest := Invoice{Status: "draft", Customer: source.Customer, CustomerVersion: source.CustomerVersion, InvoiceDate: input.InvoiceDate, PaymentTerm: input.PaymentTerm, CreditDays: input.CreditDays, Currency: input.Currency, PublicNotes: input.PublicNotes, DiscountType: input.DiscountType, Discount: input.Discount, Exoneration: input.Exoneration, Calculation: calculated.Calculation, PaidTotal: "0.00", Balance: calculated.Calculation.Total, Version: 1, CreatedBy: a.UserID, TeamID: source.TeamID, Items: []Item{}}
			for _, x := range calculated.Items {
				dest.Items = append(dest.Items, Item{CalculatedItem: x})
			}
			result, e := s.persist(ctx, tx, a, &dest)
			if e != nil {
				return nil, e
			}
			if _, e = tx.ExecContext(ctx, `INSERT INTO invoice_clones(invoice_id,source_invoice_id,source_version,created_by,created_at) VALUES(?,?,?,?,UTC_TIMESTAMP(6))`, dest.ID, source.ID, source.Version, a.UserID); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "invoice.cloned", "invoices", dest.ID, "success", map[string]string{"source_id": source.ID, "source_version": fmt.Sprint(source.Version)}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "invoice.cloned", "invoices", dest.ID, dest.Version); e != nil {
				return nil, e
			}
			return result, nil
		})
		if e != nil {
			return e
		}
		raw, e := json.Marshal(out)
		if e != nil {
			return e
		}
		var result struct {
			ID string `json:"id"`
		}
		if e = json.Unmarshal(raw, &result); e != nil {
			return e
		}
		clone, e := s.access(ctx, tx, a, result.ID, "facturas.view", true)
		if e != nil {
			return e
		}
		return s.Core.Authorize(ctx, tx, a, "facturas.create", clone.CreatedBy, clone.TeamID, false)
	})
	return out, dbError(e)
}
