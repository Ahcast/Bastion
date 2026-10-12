package invoices

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"time"
)

type PurchaseOrderSource interface {
	LockInvoiceSource(context.Context, *sql.Tx, core.Actor, string, string) (purchaseorders.PurchaseOrder, error)
	LockInvoiceLedger(context.Context, *sql.Tx, core.Actor, string, string, bool) (bool, error)
	MarkInvoiceIssued(context.Context, *sql.Tx, core.Actor, string, string, string, uint64) error
}

func (s *Service) AuthorizePurchaseOrderInvoice(ctx context.Context, tx *sql.Tx, a core.Actor, team string, existing *string) error {
	return s.AuthorizeQuoteInvoice(ctx, tx, a, team, existing)
}
func inputFromOrder(o purchaseorders.PurchaseOrder, p purchaseorders.InvoiceConversionInput) Input {
	out := Input{Customer: CustomerInput{ID: o.Customer.ID, Name: o.Customer.Name, RTN: o.Customer.RTN, Contact: o.Customer.Contact, Phone: o.Customer.Phone, Email: o.Customer.Email, Address: o.Customer.Address}, InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, CreditDays: p.CreditDays, Currency: o.Currency, PublicNotes: p.PublicNotes, DiscountType: o.DiscountType, Discount: o.Discount, Exoneration: o.Exoneration, AdditionalExpenses: o.Calculation.AdditionalExpenses, TeamID: o.TeamID}
	for _, x := range o.Items {
		out.Items = append(out.Items, ItemInput{Item: finance.Item{Description: x.Description, Unit: x.Unit, Quantity: x.Quantity, Price: x.UnitSalePrice, Cost: x.UnitCost, DiscountType: x.DiscountType, Discount: x.Discount, Tax: x.TaxType}})
	}
	return out
}
func financialsFromOrder(c purchaseorders.Calculation) Financials {
	return Financials{Gross: c.Gross, ItemDiscount: c.ItemDiscount, GeneralDiscount: c.GeneralDiscount, Base15: c.Base15, Base18: c.Base18, Exempt: c.Exempt, Exonerated: c.Exonerated, ISV15: c.ISV15, ISV18: c.ISV18, Total: c.Total, ProductsInvestment: c.ProductsInvestment, AdditionalExpenses: c.AdditionalExpenses, InvestmentTotal: c.InvestmentTotal, Profit: c.Profit, CalculatorVersion: c.CalculatorVersion, ProfitVersion: c.ProfitVersion}
}
func (s *Service) CreateFromPurchaseOrder(ctx context.Context, tx *sql.Tx, a core.Actor, o purchaseorders.PurchaseOrder, meta purchaseorders.InvoiceConversionInput) (string, error) {
	if o.Deleted != nil || o.Status != purchaseorders.StatusDelivered || o.InvoiceID != nil || o.Version == 0 || o.Version != meta.Version || len(o.Items) == 0 {
		return "", core.Conflict
	}
	for _, x := range o.Items {
		if x.Status != purchaseorders.ItemStatusDelivered {
			return "", core.Conflict
		}
	}
	if e := s.AuthorizePurchaseOrderInvoice(ctx, tx, a, o.TeamID, nil); e != nil {
		return "", e
	}
	p := inputFromOrder(o, meta)
	r, e := Validate(p)
	if e != nil {
		return "", e
	}
	if r.Calculation != financialsFromOrder(o.Calculation) {
		return "", core.Conflict
	}
	inv := Invoice{Status: "draft", Customer: p.Customer, CustomerVersion: o.CustomerVersion, InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, CreditDays: p.CreditDays, Currency: p.Currency, PublicNotes: p.PublicNotes, DiscountType: p.DiscountType, Discount: p.Discount, Exoneration: p.Exoneration, Calculation: r.Calculation, PaidTotal: "0.00", Balance: r.Calculation.Total, Version: 1, CreatedBy: a.UserID, TeamID: p.TeamID, SourcePurchaseOrderID: &o.ID, SourcePurchaseOrderVersion: &o.Version}
	for _, x := range r.Items {
		inv.Items = append(inv.Items, Item{CalculatedItem: x})
	}
	if _, e = s.persist(ctx, tx, a, &inv); e != nil {
		return "", e
	}
	return inv.ID, nil
}

// Source identity is immutable. Read/authorize it without an Invoice row lock,
// acquire the source owner's lock, then lock/re-authorize the Invoice.
func (s *Service) lockSourceInvoice(ctx context.Context, tx *sql.Tx, a core.Actor, id, permission, mode string) (Invoice, *purchaseorders.PurchaseOrder, error) {
	pre, e := s.access(ctx, tx, a, id, permission, false)
	if e != nil {
		return pre, nil, e
	}
	if e = s.Core.Authorize(ctx, tx, a, "facturas.view", pre.CreatedBy, pre.TeamID, false); e != nil {
		if e == core.Forbidden {
			e = core.Missing
		}
		return pre, nil, e
	}
	var source *purchaseorders.PurchaseOrder
	var closed *bool
	if pre.SourcePurchaseOrderID != nil {
		if s.PurchaseOrders == nil {
			return pre, nil, core.Disabled
		}
		if mode == "emit" {
			o, e := s.PurchaseOrders.LockInvoiceSource(ctx, tx, a, *pre.SourcePurchaseOrderID, id)
			if e != nil {
				return pre, nil, e
			}
			source = &o
		} else {
			value, err := s.PurchaseOrders.LockInvoiceLedger(ctx, tx, a, *pre.SourcePurchaseOrderID, id, mode == "payment")
			closed = &value
			if e = err; e != nil {
				return pre, nil, e
			}
		}
	}
	o, e := s.access(ctx, tx, a, id, permission, true)
	if e != nil {
		return o, source, e
	}
	if (pre.SourcePurchaseOrderID == nil) != (o.SourcePurchaseOrderID == nil) || pre.SourcePurchaseOrderID != nil && *pre.SourcePurchaseOrderID != *o.SourcePurchaseOrderID {
		return o, source, core.Conflict
	}
	o.SourceOrderClosed = closed
	return o, source, nil
}
func validateOrderEmission(o Invoice, source purchaseorders.PurchaseOrder) error {
	if source.Deleted != nil || source.Status != purchaseorders.StatusDelivered || source.InvoiceID == nil || *source.InvoiceID != o.ID || o.SourceQuoteID != nil || o.SourcePurchaseOrderVersion == nil || o.CustomerVersion != source.CustomerVersion || o.TeamID != source.TeamID {
		return core.Conflict
	}
	for _, x := range source.Items {
		if x.Status != purchaseorders.ItemStatusDelivered {
			return core.Conflict
		}
	}
	p := inputFromOrder(source, purchaseorders.InvoiceConversionInput{InvoiceDate: o.InvoiceDate, PaymentTerm: o.PaymentTerm, CreditDays: o.CreditDays, PublicNotes: o.PublicNotes})
	r, e := Validate(p)
	if e != nil {
		return e
	}
	if p.Customer != o.Customer || r.Currency != o.Currency || r.Calculation != o.Calculation || len(r.Items) != len(o.Items) || r.Calculation != financialsFromOrder(source.Calculation) {
		return core.Conflict
	}
	for i, x := range r.Items {
		if x != o.Items[i].CalculatedItem {
			return core.Conflict
		}
	}
	return nil
}
func (s *Service) ReadForPurchaseOrder(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (purchaseorders.LinkedInvoice, error) {
	if e := s.gate(ctx, tx, a, "facturas.view"); e != nil {
		return purchaseorders.LinkedInvoice{}, e
	}
	o, e := s.access(ctx, tx, a, id, "facturas.view", true)
	if e != nil {
		return purchaseorders.LinkedInvoice{}, e
	}
	if _, e = s.checkedPaymentProjection(ctx, tx, o); e != nil {
		return purchaseorders.LinkedInvoice{}, e
	}
	if e = invoiceCommercial(&o, time.Now().UTC()); e != nil {
		return purchaseorders.LinkedInvoice{}, e
	}
	return purchaseorders.LinkedInvoice{ID: o.ID, Version: o.Version, Number: o.Number, Status: o.Status, Currency: o.Currency, Total: o.Calculation.Total, PaidTotal: o.PaidTotal, Balance: o.Balance, SourcePurchaseOrderID: o.SourcePurchaseOrderID}, nil
}
