package invoices

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"testing"
)

func mixedInvoice(currency string) CalculationInput {
	p := CalculationInput{Currency: currency, AdditionalExpenses: "9.99", CalculationInput: finance.CalculationInput{DiscountType: "percent", Discount: "5", Exoneration: "Exoneración sintética"}}
	for _, tax := range []string{"isv15", "isv18", "exempt", "exonerated"} {
		p.Items = append(p.Items, finance.Item{Description: "Equipo sintético", Unit: "u", Quantity: "2", Price: "100", Cost: "30.005", DiscountType: "amount", Discount: "10", Tax: tax})
	}
	return p
}

func TestInvoiceFinancialsMixedTaxesDiscountsInvestmentAndCurrencies(t *testing.T) {
	for _, currency := range []string{"HNL", "USD"} {
		p := mixedInvoice(currency)
		r, e := Calculate(p)
		if e != nil {
			t.Fatal(e)
		}
		c := r.Calculation
		if r.Currency != currency || c.Gross != "800.00" || c.ItemDiscount != "80.00" || c.GeneralDiscount != "36.00" || c.Base15 != "171.00" || c.Base18 != "171.00" || c.Exempt != "171.00" || c.Exonerated != "171.00" || c.ISV15 != "25.65" || c.ISV18 != "30.78" || c.Total != "740.43" {
			t.Fatal(r)
		}
		if c.ProductsInvestment != "240.04" || c.AdditionalExpenses != "9.99" || c.InvestmentTotal != "250.03" || c.Profit != "433.97" || c.CalculatorVersion != 1 || c.ProfitVersion != 1 {
			t.Fatal(c)
		}
		if len(r.Items) != 4 || r.Items[0].LineCost != "60.01" || r.Items[0].AllocatedDiscount != "9.00" {
			t.Fatal(r.Items)
		}
		if p.Items[0].Description != "Equipo sintético" || p.AdditionalExpenses != "9.99" {
			t.Fatal("input mutated")
		}
	}
}

func TestInvoiceFinancialsLossAndInvalidBoundaries(t *testing.T) {
	p := mixedInvoice("HNL")
	p.AdditionalExpenses = "500"
	r, e := Calculate(p)
	if e != nil || r.Calculation.Profit != "-56.04" {
		t.Fatal(r, e)
	}
	for _, currency := range []string{"", "hnl", "EUR", "HNL/USD"} {
		p := mixedInvoice(currency)
		if _, e := Calculate(p); e != core.Invalid {
			t.Fatal(currency, e)
		}
	}
	for _, value := range []string{"", "-1", "0.001", "1e2", "NaN", "9999999999999999.99"} {
		p := mixedInvoice("USD")
		p.AdditionalExpenses = value
		if _, e := Calculate(p); e != core.Invalid {
			t.Fatal(value, e)
		}
	}
	p = mixedInvoice("HNL")
	p.Exoneration = ""
	if _, e := Calculate(p); e != core.Invalid {
		t.Fatal(e)
	}
	p = mixedInvoice("HNL")
	p.Items = nil
	if _, e := Calculate(p); e != core.Invalid {
		t.Fatal(e)
	}
}

func TestInvoicePaymentsExactAbonosAndCompletion(t *testing.T) {
	for _, currency := range []string{"HNL", "USD"} {
		out, e := ProjectPayments(currency, "100.10", []string{"0.10", "25.00", "25"})
		if e != nil || out.Currency != currency || out.PaidTotal != "50.10" || out.Balance != "50.00" {
			t.Fatal(out, e)
		}
		done, e := ApplyPayment(currency, "100.10", out.PaidTotal, "50")
		if e != nil || done.PaidTotal != "100.10" || done.Balance != "0.00" {
			t.Fatal(done, e)
		}
		if _, e = ApplyPayment(currency, "100.10", done.PaidTotal, "0.01"); e != core.Conflict {
			t.Fatal(e)
		}
		// Above the integer precision limit of IEEE-754, the cent remains exact.
		big, e := ApplyPayment(currency, "9007199254740993.01", "9007199254740993.00", "0.01")
		if e != nil || big.PaidTotal != "9007199254740993.01" || big.Balance != "0.00" {
			t.Fatal(big, e)
		}
	}
	for _, total := range []string{"0", "100.10"} {
		out, e := ProjectPayments("HNL", total, nil)
		if e != nil || out.PaidTotal != "0.00" {
			t.Fatal(out, e)
		}
		if total == "0" && out.Balance != "0.00" || total == "100.10" && out.Balance != "100.10" {
			t.Fatal(out)
		}
	}
}

func TestInvoicePaymentsRejectMalformedOrIncoherentLedger(t *testing.T) {
	for _, value := range []string{"", "0", "0.00", "-1", "0.001", "1e2", "NaN", "99999999999999999.00"} {
		if _, e := ApplyPayment("HNL", "100", "0", value); e != core.Invalid {
			t.Fatal(value, e)
		}
	}
	for _, args := range [][4]string{{"EUR", "100", "0", "1"}, {"HNL", "-1", "0", "1"}, {"HNL", "100", "-1", "1"}, {"HNL", "100", "0.001", "1"}} {
		if _, e := ApplyPayment(args[0], args[1], args[2], args[3]); e != core.Invalid {
			t.Fatal(args, e)
		}
	}
	for _, amounts := range [][]string{{"100.01"}, {"50", "50.01"}} {
		if _, e := ProjectPayments("USD", "100", amounts); e != core.Conflict {
			t.Fatal(amounts, e)
		}
	}
	if _, e := ApplyPayment("HNL", "100", "101", "1"); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e := ProjectPayments("HNL", "100", []string{"50", "-1"}); e != core.Invalid {
		t.Fatal(e)
	}
}
