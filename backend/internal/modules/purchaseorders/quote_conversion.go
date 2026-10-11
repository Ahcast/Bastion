package purchaseorders

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func (s *Service) AuthorizeQuoteConversion(ctx context.Context, tx *sql.Tx, a core.Actor, team string, existing *string) error {
	if a.Type != "user" {
		return core.Forbidden
	}
	if e := s.gate(ctx, tx, a, "ordenes.create"); e != nil {
		return e
	}
	if e := s.authorizeCreateTeam(ctx, tx, a, "ordenes.create", team); e != nil {
		return e
	}
	if existing != nil {
		_, e := s.access(ctx, tx, a, *existing, "ordenes.view", true)
		return e
	}
	return s.Core.Authorize(ctx, tx, a, "ordenes.view", a.UserID, team, false)
}

func (s *Service) CreateFromQuote(ctx context.Context, tx *sql.Tx, a core.Actor, q quotes.Quote, meta quotes.PurchaseOrderConversionInput) (string, error) {
	if q.Status != "accepted" || q.Deleted != nil || core.ID(q.ID) != nil || q.Version == 0 || meta.Version != q.Version {
		return "", core.Conflict
	}
	if e := s.AuthorizeQuoteConversion(ctx, tx, a, q.Team, nil); e != nil {
		return "", e
	}
	items := make([]Item, 0, len(q.Items))
	for _, x := range q.Items {
		items = append(items, Item{Description: x.Description, Unit: x.Unit, Quantity: x.Quantity, UnitSalePrice: x.Price, UnitCost: x.Cost, DiscountType: x.DiscountType, Discount: x.Discount, TaxType: x.Tax})
	}
	p := Input{OrderNumber: meta.OrderNumber, Customer: CustomerInput{ID: q.Customer.ID, Name: q.Customer.Name, RTN: q.Customer.RTN, Contact: q.Customer.Contact, Phone: q.Customer.Phone, Email: q.Customer.Email, Address: q.Customer.Address}, Destination: meta.Destination, OrderDate: meta.OrderDate, RequiredDeliveryDate: meta.RequiredDeliveryDate, Currency: q.Currency, Comments: meta.Comments, TeamID: q.Team, CalculationInput: CalculationInput{Items: items, DiscountType: q.DiscountType, Discount: q.Discount, Exoneration: q.Exoneration, AdditionalExpenses: meta.AdditionalExpenses}}
	calculated, e := Validate(&p)
	if e != nil {
		return "", e
	}
	c := calculated.Calculation
	source := q.Calculation
	if c.Gross != source.Gross || c.ItemDiscount != source.ItemDiscount || c.GeneralDiscount != source.GeneralDiscount || c.Base15 != source.Base15 || c.Base18 != source.Base18 || c.Exempt != source.Exempt || c.Exonerated != source.Exonerated || c.ISV15 != source.ISV15 || c.ISV18 != source.ISV18 || c.Total != source.Total || c.ProductsInvestment != source.Cost || c.CalculatorVersion != source.CalculatorVersion {
		return "", core.Conflict
	}
	o := PurchaseOrder{OrderNumber: p.OrderNumber, Status: "received", Customer: p.Customer, CustomerVersion: q.CustomerVersion, SourceQuoteID: &q.ID, SourceQuoteVersion: &q.Version, Destination: p.Destination, OrderDate: p.OrderDate, RequiredDeliveryDate: p.RequiredDeliveryDate, Currency: p.Currency, Comments: p.Comments, DiscountType: p.DiscountType, Discount: p.Discount, Exoneration: p.Exoneration, Calculation: c, Version: 1, CreatedBy: a.UserID, TeamID: q.Team, Items: []PurchaseOrderItem{}}
	for _, x := range calculated.Items {
		o.Items = append(o.Items, PurchaseOrderItem{CalculatedItem: x, Status: "pending"})
	}
	if _, e = s.persist(ctx, tx, a, &o); e != nil {
		return "", e
	}
	return o.ID, nil
}
