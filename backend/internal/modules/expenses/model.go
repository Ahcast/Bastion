// Package expenses owns expenses, exact legacy tax calculation and reversible deletion.
package expenses

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
	"time"
)

var Permissions = []string{"gastos.view", "gastos.create", "gastos.update", "gastos.delete", "gastos.restore"}
var Categories = []string{"product_purchase", "freight", "customs", "taxes", "transport", "travel", "software", "services", "advertising", "office", "other"}

func category(v string) bool {
	for _, x := range Categories {
		if x == v {
			return true
		}
	}
	return false
}
func date(v string) bool {
	d, e := time.Parse("2006-01-02", v)
	return e == nil && d.Format("2006-01-02") == v && d.Year() >= 1000
}

type CalculationInput struct {
	Currency string `json:"currency"`
	Taxable  string `json:"taxable_amount"`
	Exempt   string `json:"exempt_amount"`
}
type Calculation struct {
	Currency          string `json:"currency"`
	Taxable           string `json:"taxable_amount"`
	Exempt            string `json:"exempt_amount"`
	Rate              string `json:"tax_rate"`
	Tax               string `json:"tax_amount"`
	Total             string `json:"total"`
	CalculatorVersion int    `json:"calculator_version"`
}

func Calculate(p CalculationInput) (Calculation, error) {
	var out Calculation
	if p.Currency != "HNL" && p.Currency != "USD" {
		return out, core.Invalid
	}
	taxable, e := finance.ParseDecimal(p.Taxable, 2)
	if e != nil {
		return out, e
	}
	exempt, e := finance.ParseDecimal(p.Exempt, 2)
	if e != nil {
		return out, e
	}
	tax := taxable.Mul(decimal.RequireFromString("0.15")).Round(2)
	total := taxable.Add(exempt).Add(tax)
	if _, e = finance.ParseDecimal(total.StringFixed(2), 2); e != nil {
		return out, core.Invalid
	}
	return Calculation{Currency: p.Currency, Taxable: taxable.StringFixed(2), Exempt: exempt.StringFixed(2), Rate: "0.1500", Tax: tax.StringFixed(2), Total: total.StringFixed(2), CalculatorVersion: 1}, nil
}

type Supplier struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}
type Input struct {
	InvoiceNumber string   `json:"invoice_number"`
	Date          string   `json:"expense_date"`
	Supplier      Supplier `json:"supplier"`
	Category      string   `json:"category"`
	CalculationInput
	Notes   string `json:"notes"`
	TeamID  string `json:"team_id"`
	Version uint64 `json:"version,string"`
}
type Expense struct {
	ID string `json:"id"`
	Input
	SupplierVersion uint64      `json:"supplier_version,string"`
	Calculation     Calculation `json:"calculation"`
	CreatedBy       string      `json:"created_by"`
	UpdatedBy       string      `json:"updated_by"`
	CreatedAt       time.Time   `json:"created_at"`
	UpdatedAt       time.Time   `json:"updated_at"`
	DeletedAt       *time.Time  `json:"deleted_at"`
	DeletedBy       *string     `json:"deleted_by"`
	DeleteReason    *string     `json:"delete_reason"`
}
type StateInput struct {
	Version uint64 `json:"version,string"`
	Reason  string `json:"reason"`
}

func Validate(p Input) (Calculation, error) {
	if !date(p.Date) || !category(p.Category) || !finance.ValidText(p.InvoiceNumber, 100, false) || !finance.ValidText(p.Supplier.Name, 1000, p.Supplier.ID == "") || p.Supplier.ID != "" && core.ID(p.Supplier.ID) != nil || !finance.ValidText(p.Notes, 12000, false) || p.TeamID != "" && core.ID(p.TeamID) != nil {
		return Calculation{}, core.Invalid
	}
	return Calculate(p.CalculationInput)
}
