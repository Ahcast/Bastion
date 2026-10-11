// Package invoices owns invoice financials and payment arithmetic. Commercial
// actions must authorize and persist these results through the invoice owner.
package invoices

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
)

type CalculationInput struct {
	Currency string `json:"currency"`
	finance.CalculationInput
	AdditionalExpenses string `json:"additional_expenses"`
}

type Financials struct {
	Gross              string `json:"gross"`
	ItemDiscount       string `json:"item_discount"`
	GeneralDiscount    string `json:"general_discount"`
	Base15             string `json:"base_15"`
	Base18             string `json:"base_18"`
	Exempt             string `json:"exempt"`
	Exonerated         string `json:"exonerated"`
	ISV15              string `json:"isv_15"`
	ISV18              string `json:"isv_18"`
	Total              string `json:"total"`
	ProductsInvestment string `json:"products_investment"`
	AdditionalExpenses string `json:"additional_expenses"`
	InvestmentTotal    string `json:"investment_total"`
	Profit             string `json:"profit"`
	CalculatorVersion  int    `json:"calculator_version"`
	ProfitVersion      int    `json:"profit_version"`
}

type CalculationResult struct {
	Currency    string                   `json:"currency"`
	Items       []finance.CalculatedItem `json:"items"`
	Calculation Financials               `json:"calculation"`
}

func validCurrency(currency string) bool { return currency == "HNL" || currency == "USD" }

func Calculate(p CalculationInput) (CalculationResult, error) {
	var out CalculationResult
	if !validCurrency(p.Currency) || len(p.Items) == 0 || len(p.Items) > 200 {
		return out, core.Invalid
	}
	sales, e := finance.Calculate(p.CalculationInput)
	if e != nil {
		return out, e
	}
	additional, e := finance.ParseDecimal(p.AdditionalExpenses, 2)
	if e != nil {
		return out, e
	}
	products, e := finance.ParseDecimal(sales.Calculation.Cost, 2)
	if e != nil {
		return out, e
	}
	investment := products.Add(additional).StringFixed(2)
	c := sales.Calculation
	profit, e := finance.Profit(c.Base15, c.Base18, c.Exempt, c.Exonerated, investment)
	if e != nil {
		return out, e
	}
	return CalculationResult{Currency: p.Currency, Items: sales.Items, Calculation: Financials{
		Gross: c.Gross, ItemDiscount: c.ItemDiscount, GeneralDiscount: c.GeneralDiscount,
		Base15: c.Base15, Base18: c.Base18, Exempt: c.Exempt, Exonerated: c.Exonerated,
		ISV15: c.ISV15, ISV18: c.ISV18, Total: c.Total, ProductsInvestment: products.StringFixed(2),
		AdditionalExpenses: additional.StringFixed(2), InvestmentTotal: investment, Profit: profit,
		CalculatorVersion: c.CalculatorVersion, ProfitVersion: 1,
	}}, nil
}

type PaymentProjection struct {
	Currency  string `json:"currency"`
	PaidTotal string `json:"paid_total"`
	Balance   string `json:"balance"`
}

// ApplyPayment does arithmetic only. It neither records nor approves a payment.
func ApplyPayment(currency, total, paid, amount string) (PaymentProjection, error) {
	var out PaymentProjection
	if !validCurrency(currency) {
		return out, core.Invalid
	}
	t, e := finance.ParseDecimal(total, 2)
	if e != nil {
		return out, e
	}
	p, e := finance.ParseDecimal(paid, 2)
	if e != nil {
		return out, e
	}
	a, e := finance.ParseDecimal(amount, 2)
	if e != nil || !a.GreaterThan(decimal.Zero) {
		return out, core.Invalid
	}
	if p.GreaterThan(t) || p.Add(a).GreaterThan(t) {
		return out, core.Conflict
	}
	next := p.Add(a)
	return PaymentProjection{Currency: currency, PaidTotal: next.StringFixed(2), Balance: t.Sub(next).StringFixed(2)}, nil
}

// ProjectPayments accepts active amounts from the owner's ledger, all in the
// invoice currency. The owner retains responsibility for authorization/locking.
func ProjectPayments(currency, total string, activeAmounts []string) (PaymentProjection, error) {
	if !validCurrency(currency) {
		return PaymentProjection{}, core.Invalid
	}
	t, e := finance.ParseDecimal(total, 2)
	if e != nil {
		return PaymentProjection{}, e
	}
	out := PaymentProjection{Currency: currency, PaidTotal: "0.00", Balance: t.StringFixed(2)}
	for _, amount := range activeAmounts {
		out, e = ApplyPayment(currency, total, out.PaidTotal, amount)
		if e != nil {
			return PaymentProjection{}, e
		}
	}
	return out, nil
}
