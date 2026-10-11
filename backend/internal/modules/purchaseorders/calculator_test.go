package purchaseorders

import (
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func orderItem(quantity, sale, cost, discount, tax string) Item {
	return Item{
		Description:   "Equipo sintético",
		Unit:          "unidad",
		Quantity:      quantity,
		UnitSalePrice: sale,
		UnitCost:      cost,
		DiscountType:  "amount",
		Discount:      discount,
		TaxType:       tax,
	}
}

func TestCalculateSharesSalesRulesAndAddsInvestment(t *testing.T) {
	r, e := Calculate(CalculationInput{
		Items: []Item{
			orderItem("2", "100", "30.005", "10", "isv15"),
			orderItem("1", "100", "10", "0", "exempt"),
		},
		DiscountType:       "amount",
		Discount:           "0",
		AdditionalExpenses: "9.99",
	})
	if e != nil {
		t.Fatal(e)
	}
	c := r.Calculation
	if c.Gross != "300.00" || c.ItemDiscount != "20.00" || c.GeneralDiscount != "0.00" || c.Base15 != "180.00" || c.Exempt != "100.00" || c.ISV15 != "27.00" || c.Total != "307.00" {
		t.Fatal(c)
	}
	if c.ProductsInvestment != "70.01" || c.AdditionalExpenses != "9.99" || c.InvestmentTotal != "80.00" || c.CalculatorVersion != 1 {
		t.Fatal(c)
	}
	if r.Items[0].LineInvestment != "60.01" || r.Items[0].UnitSalePrice != "100" || r.Items[0].UnitCost != "30.005" || r.Items[0].UnitDiscount != "10" {
		t.Fatal(r.Items[0])
	}
}

func TestCalculateRejectsInvalidOrderInvestmentInputs(t *testing.T) {
	if _, e := Calculate(CalculationInput{DiscountType: "amount", Discount: "0", AdditionalExpenses: "0"}); e != core.Invalid {
		t.Fatal(e)
	}
	item := orderItem("1", "1", "1", "0", "exempt")
	for _, value := range []string{"", "-1", "0.001", "1e2", "9999999999999999.99"} {
		_, e := Calculate(CalculationInput{Items: []Item{item}, DiscountType: "amount", Discount: "0", AdditionalExpenses: value})
		if e != core.Invalid {
			t.Fatalf("additional_expenses=%q: %v", value, e)
		}
	}
	item.TaxType = "exonerated"
	if _, e := Calculate(CalculationInput{Items: []Item{item}, DiscountType: "amount", Discount: "0", AdditionalExpenses: "0"}); e != core.Invalid {
		t.Fatal(e)
	}
}
