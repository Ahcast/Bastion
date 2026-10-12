package invoices

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func (s *Service) AuthorizeQuoteInvoice(ctx context.Context, tx *sql.Tx, a core.Actor, team string, existing *string) error {
	if a.Type != "user" {
		return core.Forbidden
	}
	if e := s.gate(ctx, tx, a, "facturas.create"); e != nil {
		return e
	}
	if e := s.destination(ctx, tx, a, "facturas.create", a.UserID, team); e != nil {
		return e
	}
	if existing != nil {
		_, e := s.access(ctx, tx, a, *existing, "facturas.view", true)
		return e
	}
	return nil
}
func (s *Service) CreateFromQuote(ctx context.Context, tx *sql.Tx, a core.Actor, q quotes.Quote, meta quotes.InvoiceConversionInput) (string, error) {
	if q.Status != "accepted" || q.Deleted != nil || core.ID(q.ID) != nil || q.Version == 0 || q.Version != meta.Version || q.InvoiceID != nil || q.PurchaseOrderID != nil {
		return "", core.Conflict
	}
	if e := s.AuthorizeQuoteInvoice(ctx, tx, a, q.Team, nil); e != nil {
		return "", e
	}
	p := Input{Customer: CustomerInput{ID: q.Customer.ID, Name: q.Customer.Name, RTN: q.Customer.RTN, Contact: q.Customer.Contact, Phone: q.Customer.Phone, Email: q.Customer.Email, Address: q.Customer.Address}, InvoiceDate: meta.InvoiceDate, PaymentTerm: meta.PaymentTerm, CreditDays: meta.CreditDays, Currency: q.Currency, PublicNotes: meta.PublicNotes, DiscountType: q.DiscountType, Discount: q.Discount, Exoneration: q.Exoneration, AdditionalExpenses: meta.AdditionalExpenses, TeamID: q.Team}
	for _, x := range q.Items {
		p.Items = append(p.Items, ItemInput{Item: x.Item})
	}
	calculated, e := Validate(p)
	if e != nil {
		return "", e
	}
	c, source := calculated.Calculation, q.Calculation
	if c.Gross != source.Gross || c.ItemDiscount != source.ItemDiscount || c.GeneralDiscount != source.GeneralDiscount || c.Base15 != source.Base15 || c.Base18 != source.Base18 || c.Exempt != source.Exempt || c.Exonerated != source.Exonerated || c.ISV15 != source.ISV15 || c.ISV18 != source.ISV18 || c.Total != source.Total || c.ProductsInvestment != source.Cost || c.CalculatorVersion != source.CalculatorVersion {
		return "", core.Conflict
	}
	o := Invoice{Status: "draft", Customer: p.Customer, CustomerVersion: q.CustomerVersion, InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, CreditDays: p.CreditDays, Currency: p.Currency, PublicNotes: p.PublicNotes, DiscountType: p.DiscountType, Discount: p.Discount, Exoneration: p.Exoneration, Calculation: c, PaidTotal: "0.00", Balance: c.Total, Version: 1, CreatedBy: a.UserID, TeamID: q.Team, SourceQuoteID: &q.ID, SourceQuoteVersion: &q.Version}
	for _, x := range calculated.Items {
		o.Items = append(o.Items, Item{CalculatedItem: x})
	}
	if _, e = s.persist(ctx, tx, a, &o); e != nil {
		return "", e
	}
	return o.ID, nil
}
