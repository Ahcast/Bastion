package purchaseorders

import (
	"net/mail"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
)

var Permissions = []string{"ordenes.view", "ordenes.create", "ordenes.update"}

type CustomerInput struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	RTN     string `json:"rtn"`
	Contact string `json:"contact"`
	Phone   string `json:"phone"`
	Email   string `json:"email"`
	Address string `json:"address"`
}

type Input struct {
	OrderNumber          string        `json:"order_number"`
	Customer             CustomerInput `json:"customer"`
	Destination          string        `json:"destination"`
	OrderDate            string        `json:"order_date"`
	RequiredDeliveryDate *string       `json:"required_delivery_date"`
	Currency             string        `json:"currency"`
	Comments             string        `json:"comments"`
	CalculationInput
	TeamID  string `json:"team_id"`
	Version uint64 `json:"version,string"`
}

type PurchaseOrderItem struct {
	CalculatedItem
	SupplierVersion uint64 `json:"supplier_version,string"`
	Status          string `json:"status"`
}

type PurchaseOrder struct {
	ID                   string              `json:"id"`
	OrderNumber          string              `json:"order_number"`
	Status               string              `json:"status"`
	Customer             CustomerInput       `json:"customer"`
	CustomerVersion      uint64              `json:"customer_version,string"`
	SourceQuoteID        *string             `json:"source_quote_id"`
	Destination          string              `json:"destination"`
	OrderDate            string              `json:"order_date"`
	RequiredDeliveryDate *string             `json:"required_delivery_date"`
	Currency             string              `json:"currency"`
	Comments             string              `json:"comments"`
	DiscountType         string              `json:"general_discount_type"`
	Discount             string              `json:"general_discount"`
	Exoneration          string              `json:"exoneration_reference"`
	Calculation          Calculation         `json:"calculation"`
	Version              uint64              `json:"version,string"`
	CreatedBy            string              `json:"created_by"`
	TeamID               string              `json:"team_id"`
	Items                []PurchaseOrderItem `json:"items"`
}

func validEmail(v string) bool {
	if v == "" {
		return true
	}
	m, e := mail.ParseAddress(v)
	return e == nil && m.Address == v && len(v) <= 254 && !strings.ContainsAny(v, "\r\n")
}

func validDate(v string) bool {
	d, e := time.Parse("2006-01-02", v)
	return e == nil && d.Year() >= 1900 && d.Year() <= 9999
}

func Validate(p *Input) (CalculationResult, error) {
	var zero CalculationResult
	p.Items = append([]Item{}, p.Items...)
	p.OrderNumber = strings.TrimSpace(p.OrderNumber)

	if !finance.ValidText(p.OrderNumber, 160, true) ||
		!finance.ValidText(p.Customer.Name, 1000, true) ||
		!finance.ValidText(p.Customer.RTN, 30, false) ||
		!finance.ValidText(p.Customer.Contact, 120, false) ||
		!finance.ValidText(p.Customer.Phone, 50, false) ||
		!validEmail(p.Customer.Email) ||
		!finance.ValidText(p.Customer.Address, 1000, false) ||
		!finance.ValidText(p.Destination, 1000, false) ||
		!finance.ValidText(p.Comments, 12000, false) ||
		!finance.ValidText(p.Exoneration, 500, false) {
		return zero, core.Invalid
	}
	if p.Customer.ID != "" && core.ID(p.Customer.ID) != nil {
		return zero, core.Invalid
	}
	if _, e := customers.CanonicalRTN(p.Customer.RTN); e != nil {
		return zero, e
	}
	if !validDate(p.OrderDate) {
		return zero, core.Invalid
	}
	if p.RequiredDeliveryDate != nil && !validDate(*p.RequiredDeliveryDate) {
		return zero, core.Invalid
	}
	if p.Currency != "HNL" && p.Currency != "USD" {
		return zero, core.Invalid
	}
	if p.TeamID != "" && core.ID(p.TeamID) != nil {
		return zero, core.Invalid
	}
	for i := range p.Items {
		x := &p.Items[i]
		if x.SupplierID != "" && core.ID(x.SupplierID) != nil {
			return zero, core.Invalid
		}
		if !finance.ValidText(x.SupplierName, 180, false) ||
			!finance.ValidText(x.Tracking, 2000, false) ||
			!finance.ValidText(x.Comments, 12000, false) {
			return zero, core.Invalid
		}
	}

	r, e := Calculate(p.CalculationInput)
	if e != nil {
		return zero, e
	}
	for i := range r.Items {
		p.Items[i] = r.Items[i].Item
	}
	scale := int32(2)
	if p.DiscountType == "percent" {
		scale = 4
	}
	d, e := finance.ParseDecimal(p.Discount, scale)
	if e != nil {
		return zero, e
	}
	p.Discount = d.String()
	p.AdditionalExpenses = r.Calculation.AdditionalExpenses
	return r, nil
}
