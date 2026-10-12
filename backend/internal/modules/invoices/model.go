package invoices

import (
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"net/mail"
	"strings"
	"time"
)

var Permissions = []string{"facturas.view", "facturas.create", "facturas.update", "facturas.fiscal.read", "facturas.fiscal.manage", "facturas.emit", "facturas.payment", "facturas.payment.void"}

type CustomerInput struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	RTN     string `json:"rtn"`
	Contact string `json:"contact"`
	Phone   string `json:"phone"`
	Email   string `json:"email"`
	Address string `json:"address"`
}
type ItemInput struct {
	ID string `json:"id,omitempty"`
	finance.Item
}
type Input struct {
	Customer           CustomerInput `json:"customer"`
	InvoiceDate        string        `json:"invoice_date"`
	PaymentTerm        string        `json:"payment_term_type"`
	CreditDays         *uint16       `json:"credit_days"`
	Currency           string        `json:"currency"`
	PublicNotes        string        `json:"public_notes"`
	DiscountType       string        `json:"general_discount_type"`
	Discount           string        `json:"general_discount"`
	Exoneration        string        `json:"exoneration_reference"`
	AdditionalExpenses string        `json:"additional_expenses"`
	TeamID             string        `json:"team_id"`
	Version            uint64        `json:"version,string"`
	Items              []ItemInput   `json:"items"`
}
type Item struct {
	ID string `json:"id"`
	finance.CalculatedItem
}
type Invoice struct {
	SourceOrderClosed          *bool           `json:"source_order_closed"`
	SourcePurchaseOrderID      *string         `json:"source_purchase_order_id"`
	SourcePurchaseOrderVersion *uint64         `json:"source_purchase_order_version,string"`
	SourceQuoteID              *string         `json:"source_quote_id"`
	SourceQuoteVersion         *uint64         `json:"source_quote_version,string"`
	ID                         string          `json:"id"`
	Number                     *string         `json:"invoice_number"`
	SAR                        json.RawMessage `json:"sar_snapshot_json"`
	IssuedAt                   *time.Time      `json:"issued_at"`
	IssuedBy                   *string         `json:"issued_by"`
	DueDate                    *string         `json:"due_date"`
	Company                    json.RawMessage `json:"company_snapshot_json"`
	CompanyVersion             *uint64         `json:"company_version,string"`

	CommercialStatus *string       `json:"commercial_status"`
	Status           string        `json:"status"`
	Customer         CustomerInput `json:"customer"`
	CustomerVersion  uint64        `json:"customer_version,string"`
	InvoiceDate      string        `json:"invoice_date"`
	PaymentTerm      string        `json:"payment_term_type"`
	CreditDays       *uint16       `json:"credit_days"`
	Currency         string        `json:"currency"`
	PublicNotes      string        `json:"public_notes"`
	DiscountType     string        `json:"general_discount_type"`
	Discount         string        `json:"general_discount"`
	Exoneration      string        `json:"exoneration_reference"`
	Calculation      Financials    `json:"calculation"`
	PaidTotal        string        `json:"paid_total"`
	Balance          string        `json:"balance"`
	Version          uint64        `json:"version,string"`
	CreatedBy        string        `json:"created_by"`
	TeamID           string        `json:"team_id"`
	Items            []Item        `json:"items"`
}

func Validate(p Input) (CalculationResult, error) {
	var zero CalculationResult
	date, e := time.Parse("2006-01-02", p.InvoiceDate)
	if e != nil || date.Year() < 1900 || date.Year() > 9999 || !validCurrency(p.Currency) {
		return zero, core.Invalid
	}
	if p.PaymentTerm == "contado" {
		if p.CreditDays != nil {
			return zero, core.Invalid
		}
	} else if p.PaymentTerm == "credito" {
		if p.CreditDays == nil || *p.CreditDays == 0 {
			return zero, core.Invalid
		}
	} else {
		return zero, core.Invalid
	}
	c := p.Customer
	for _, v := range []struct {
		s        string
		n        int
		required bool
	}{{c.Name, 1000, true}, {c.RTN, 30, false}, {c.Contact, 120, false}, {c.Phone, 50, false}, {c.Email, 254, false}, {c.Address, 1000, false}, {p.PublicNotes, 12000, false}} {
		if !finance.ValidText(v.s, v.n, v.required) {
			return zero, core.Invalid
		}
	}
	if c.Email != "" {
		m, e := mail.ParseAddress(c.Email)
		if e != nil || m.Address != c.Email || strings.ContainsAny(c.Email, "\r\n") {
			return zero, core.Invalid
		}
	}
	for _, id := range []string{c.ID, p.TeamID} {
		if id != "" && core.ID(id) != nil {
			return zero, core.Invalid
		}
	}
	items := make([]finance.Item, len(p.Items))
	seen := map[string]bool{}
	for i, x := range p.Items {
		if x.ID != "" {
			if core.ID(x.ID) != nil || seen[x.ID] {
				return zero, core.Invalid
			}
			seen[x.ID] = true
		}
		items[i] = x.Item
	}
	return Calculate(CalculationInput{Currency: p.Currency, AdditionalExpenses: p.AdditionalExpenses, CalculationInput: finance.CalculationInput{Items: items, DiscountType: p.DiscountType, Discount: p.Discount, Exoneration: p.Exoneration}})
}
