package invoices

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"time"
	_ "time/tzdata"
)

type EmitInput struct {
	SourceOrderVersion *uint64 `json:"source_order_version,string"`
	Version            uint64  `json:"version,string"`
	FiscalVersion      uint64  `json:"fiscal_version,string"`
	CompanyVersion     uint64  `json:"company_version,string"`
	Reason             string  `json:"reason"`
}
type EmissionContext struct {
	SourceOrderVersion *uint64              `json:"source_order_version,string"`
	InvoiceVersion     uint64               `json:"invoice_version,string"`
	FiscalVersion      uint64               `json:"fiscal_version,string"`
	CompanyVersion     uint64               `json:"company_version,string"`
	Fiscal             FiscalAuthorization  `json:"fiscal"`
	Company            core.CompanySettings `json:"company"`
	EstimatedNumber    string               `json:"estimated_number"`
	DueDate            string               `json:"due_date"`
}
type EmissionResult struct {
	ID       string    `json:"id"`
	Version  uint64    `json:"version,string"`
	Status   string    `json:"status"`
	Number   string    `json:"invoice_number"`
	IssuedAt time.Time `json:"issued_at"`
	DueDate  string    `json:"due_date"`
}
type FiscalSnapshot struct {
	Configuration FiscalAuthorization `json:"configuration"`
	Version       uint64              `json:"configuration_version,string"`
}

func invoiceBusinessDate(now time.Time) (string, error) {
	location, e := time.LoadLocation("America/Tegucigalpa")
	if e != nil {
		return "", e
	}
	return now.In(location).Format("2006-01-02"), nil
}
func invoiceDueDate(o Invoice) (string, error) {
	if !fiscalDate(o.InvoiceDate) {
		return "", core.Invalid
	}
	d, _ := time.Parse("2006-01-02", o.InvoiceDate)
	if o.PaymentTerm == "credito" {
		if o.CreditDays == nil || *o.CreditDays == 0 {
			return "", core.Invalid
		}
		d = d.AddDate(0, 0, int(*o.CreditDays))
	} else if o.PaymentTerm != "contado" || o.CreditDays != nil {
		return "", core.Invalid
	}
	if d.Year() > 9999 {
		return "", core.Invalid
	}
	return d.Format("2006-01-02"), nil
}
func persistedInput(o Invoice) Input {
	p := Input{Customer: o.Customer, InvoiceDate: o.InvoiceDate, PaymentTerm: o.PaymentTerm, CreditDays: o.CreditDays, Currency: o.Currency, PublicNotes: o.PublicNotes, DiscountType: o.DiscountType, Discount: o.Discount, Exoneration: o.Exoneration, AdditionalExpenses: o.Calculation.AdditionalExpenses, TeamID: o.TeamID, Version: o.Version}
	for _, x := range o.Items {
		p.Items = append(p.Items, ItemInput{ID: x.ID, Item: x.Item})
	}
	return p
}
func (s *Service) emissionData(ctx context.Context, tx *sql.Tx, a core.Actor, id string, now time.Time) (EmissionContext, Invoice, FiscalAllocation, error) {
	var c EmissionContext
	var o Invoice
	var allocation FiscalAllocation
	setting, e := fiscalSettings(ctx, tx)
	if e != nil {
		return c, o, allocation, e
	}
	var sourceOrder *purchaseorders.PurchaseOrder
	o, sourceOrder, e = s.lockSourceInvoice(ctx, tx, a, id, "facturas.emit", "emit")
	if e != nil {
		return c, o, allocation, e
	}
	if e = s.Core.Authorize(ctx, tx, a, "facturas.view", o.CreatedBy, o.TeamID, false); e != nil {
		if e == core.Forbidden {
			e = core.Missing
		}
		return c, o, allocation, e
	}
	if o.Status != "draft" || o.Number != nil || o.SAR != nil || o.Version == 0 || o.Version == ^uint64(0) || o.PaidTotal != "0.00" || o.Balance != o.Calculation.Total {
		return c, o, allocation, core.Conflict
	}
	if e = s.items(ctx, tx, &o); e != nil {
		return c, o, allocation, e
	}
	recalculated, e := Validate(persistedInput(o))
	if e != nil {
		return c, o, allocation, e
	}
	if recalculated.Calculation != o.Calculation || len(recalculated.Items) != len(o.Items) {
		return c, o, allocation, core.Conflict
	}
	for i, x := range recalculated.Items {
		if x != o.Items[i].CalculatedItem {
			return c, o, allocation, core.Conflict
		}
	}
	if sourceOrder != nil {
		if e = validateOrderEmission(o, *sourceOrder); e != nil {
			return c, o, allocation, e
		}
		c.SourceOrderVersion = &sourceOrder.Version
	}
	businessDate, e := invoiceBusinessDate(now)
	if e != nil {
		return c, o, allocation, e
	}
	allocation, e = AllocateFiscalNumber(setting.Value, setting.NextValue, o.InvoiceDate, businessDate, o.Currency)
	if e != nil {
		return c, o, allocation, e
	}
	company, version, e := s.Core.PublicCompany(ctx, tx)
	if e != nil {
		return c, o, allocation, e
	}
	if version == 0 || !finance.ValidText(company.Name, 120, true) || !finance.ValidText(company.TaxID, 40, true) || !finance.ValidText(company.Address, 500, false) {
		return c, o, allocation, core.Invalid
	}
	due, e := invoiceDueDate(o)
	if e != nil {
		return c, o, allocation, e
	}
	return EmissionContext{SourceOrderVersion: c.SourceOrderVersion, InvoiceVersion: o.Version, FiscalVersion: setting.Version, CompanyVersion: version, Fiscal: setting.Value, Company: company, EstimatedNumber: allocation.Number, DueDate: due}, o, allocation, nil
}
func (s *Service) EmissionContext(ctx context.Context, a core.Actor, id string) (EmissionContext, error) {
	var out EmissionContext
	if a.Type != "user" {
		return out, core.Forbidden
	}
	if core.ID(id) != nil {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gateSensitive(ctx, tx, a, "facturas.emit", false); e != nil {
			return e
		}
		var e error
		out, _, _, e = s.emissionData(ctx, tx, a, id, time.Now().UTC())
		return e
	})
	return out, e
}
func (s *Service) Emit(ctx context.Context, a core.Actor, id, key string, p EmitInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || key == "" || p.Version == 0 || p.FiscalVersion == 0 || p.CompanyVersion == 0 || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gateSensitive(ctx, tx, a, "facturas.emit", true); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "invoices.emit:"+id, key, p, func() (any, error) {
			now := time.Now().UTC().Truncate(time.Microsecond)
			c, o, allocation, e := s.emissionData(ctx, tx, a, id, now)
			if e != nil {
				return nil, e
			}
			if o.SourcePurchaseOrderID != nil && (p.SourceOrderVersion == nil || *p.SourceOrderVersion == 0) || o.SourcePurchaseOrderID == nil && p.SourceOrderVersion != nil {
				return nil, core.Invalid
			}

			if (c.SourceOrderVersion == nil) != (p.SourceOrderVersion == nil) || c.SourceOrderVersion != nil && *c.SourceOrderVersion != *p.SourceOrderVersion {
				return nil, core.Conflict
			}
			if c.InvoiceVersion != p.Version || c.FiscalVersion != p.FiscalVersion || c.CompanyVersion != p.CompanyVersion {
				return nil, core.Conflict
			}
			sar, e := json.Marshal(FiscalSnapshot{Configuration: allocation.Snapshot, Version: c.FiscalVersion})
			if e != nil {
				return nil, e
			}
			company, e := json.Marshal(c.Company)
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE invoices SET invoice_number=?,sar_snapshot_json=?,status='issued',issued_at=?,issued_by=?,due_date=?,company_snapshot_json=?,company_version=?,version=version+1,updated_by=?,updated_at=? WHERE id=? AND version=? AND status='draft' AND invoice_number IS NULL`, allocation.Number, string(sar), now, a.UserID, c.DueDate, string(company), c.CompanyVersion, a.UserID, now, id, p.Version)); e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE document_counters SET next_value=? WHERE name='invoice' AND next_value=?`, allocation.NextValue, allocation.NextValue-1)); e != nil {
				return nil, e
			}
			if o.SourcePurchaseOrderID != nil {
				if e = s.PurchaseOrders.MarkInvoiceIssued(ctx, tx, a, *o.SourcePurchaseOrderID, id, allocation.Number, *c.SourceOrderVersion); e != nil {
					return nil, e
				}
			}

			version := o.Version + 1
			if e = s.Core.Audit(ctx, tx, a, "invoice.issued", "invoices", id, "success", map[string]string{"version": fmt.Sprint(version), "reason": p.Reason, "fiscal_version": fmt.Sprint(c.FiscalVersion), "company_version": fmt.Sprint(c.CompanyVersion)}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "invoice.issued", "invoices", id, version); e != nil {
				return nil, e
			}
			return EmissionResult{ID: id, Version: version, Status: "issued", Number: allocation.Number, IssuedAt: now, DueDate: c.DueDate}, nil
		})
		if e != nil {
			return e
		}
		current, _, e := s.lockSourceInvoice(ctx, tx, a, id, "facturas.emit", "emit")
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "facturas.view", current.CreatedBy, current.TeamID, false); e == core.Forbidden {
			return core.Missing
		}
		return e
	})
	return out, dbError(e)
}
