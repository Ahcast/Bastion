package quotes

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/shopspring/decimal"
	"testing"
)

func item(q, p, d, tax string) Item {
	return Item{Description: "Equipo sintético", Unit: "unidad", Quantity: q, Price: p, Cost: "1", DiscountType: "amount", Discount: d, Tax: tax}
}
func TestPerUnitDiscountAndFourTaxBases(t *testing.T) {
	p := CalculationInput{DiscountType: "amount", Discount: "0", Exoneration: "Constancia sintética", Items: []Item{item("2", "100", "10", "isv15"), item("1", "100", "0", "isv18"), item("1", "100", "0", "exempt"), item("1", "100", "0", "exonerated")}}
	r, e := Calculate(p)
	if e != nil {
		t.Fatal(e)
	}
	c := r.Calculation
	if c.Gross != "500.00" || c.ItemDiscount != "20.00" || c.Base15 != "180.00" || c.Base18 != "100.00" || c.Exempt != "100.00" || c.Exonerated != "100.00" || c.ISV15 != "27.00" || c.ISV18 != "18.00" || c.Total != "525.00" || c.Cost != "5.00" || c.Margin != "475.00" {
		t.Fatal(c)
	}
}
func TestPercentAndHalfUpBoundaries(t *testing.T) {
	x := item("3", "0.335", "10", "isv15")
	x.DiscountType = "percent"
	r, e := Calculate(CalculationInput{Items: []Item{x}, DiscountType: "percent", Discount: "10"})
	if e != nil {
		t.Fatal(e)
	}
	if r.Items[0].UnitDiscount != "0.0335" || r.Items[0].Gross != "1.01" || r.Items[0].ItemDiscount != "0.11" || r.Calculation.GeneralDiscount != "0.09" || r.Calculation.Base15 != "0.81" || r.Calculation.ISV15 != "0.12" || r.Calculation.Total != "0.93" {
		t.Fatal(r)
	}
	x = item("1", "0.005", "0", "exempt")
	r, e = Calculate(CalculationInput{Items: []Item{x}, DiscountType: "amount", Discount: "0"})
	if e != nil || r.Calculation.Total != "0.01" {
		t.Fatal(r, e)
	}
}
func TestLargestRemainderAllocationAndConservation(t *testing.T) {
	// Unequal remainders allocate to the larger share, stable positions break ties.
	r, e := Calculate(CalculationInput{Items: []Item{item("1", "1", "0", "isv15"), item("1", "2", "0", "isv18"), item("1", "3", "0", "exempt")}, DiscountType: "amount", Discount: "0.05"})
	if e != nil {
		t.Fatal(e)
	}
	for i, w := range []string{"0.01", "0.02", "0.02"} {
		if r.Items[i].AllocatedDiscount != w {
			t.Fatal(r)
		}
	}
	for n := 1; n <= 50; n++ {
		items := []Item{}
		for i := 0; i < n; i++ {
			items = append(items, item("1", "0.01", "0", "exempt"))
		}
		for k := 0; k <= n; k++ {
			gd := decimal.NewFromInt(int64(k)).Shift(-2).StringFixed(2)
			r, e := Calculate(CalculationInput{Items: items, DiscountType: "amount", Discount: gd})
			if e != nil {
				t.Fatal(e)
			}
			sum := decimal.Zero
			for i, x := range r.Items {
				d := decimal.RequireFromString(x.AllocatedDiscount)
				sum = sum.Add(d)
				if (i < k && x.AllocatedDiscount != "0.01") || (i >= k && x.AllocatedDiscount != "0.00") {
					t.Fatal(n, k, r)
				}
			}
			if sum.StringFixed(2) != gd || r.Calculation.Total != decimal.NewFromInt(int64(n-k)).Shift(-2).StringFixed(2) {
				t.Fatal(n, k, r)
			}
		}
	}
}
func TestFinancialInputBoundsAndExoneration(t *testing.T) {
	for _, v := range []string{"-1", "+1", "1e2", "NaN", ".5", "01", "1.0000001", "9999999999999", "1,50", " 1", ""} {
		x := item("1", v, "0", "exempt")
		if _, e := Calculate(CalculationInput{Items: []Item{x}, DiscountType: "amount", Discount: "0"}); e != core.Invalid {
			t.Fatal(v, e)
		}
	}
	bad := []Item{item("0", "1", "0", "exempt"), item("1", "1", "2", "exempt"), item("1", "1", "0", "exonerated"), item("1", "1", "0", "other"), item("99999999999999", "999999999999", "0", "exempt")}
	for _, x := range bad {
		if _, e := Calculate(CalculationInput{Items: []Item{x}, DiscountType: "amount", Discount: "0"}); e != core.Invalid {
			t.Fatal(x, e)
		}
	}
	for _, p := range []CalculationInput{{Items: []Item{item("1", "1", "0", "exempt")}, DiscountType: "amount", Discount: "1.01"}, {Items: []Item{}, DiscountType: "percent", Discount: "101"}} {
		if _, e := Calculate(p); e != core.Invalid {
			t.Fatal(p, e)
		}
	}
}
func TestEmptyDraftAndHighPrecision(t *testing.T) {
	r, e := Calculate(CalculationInput{Items: []Item{}, DiscountType: "amount", Discount: "0"})
	if e != nil || r.Items == nil || r.Calculation.Total != "0.00" {
		t.Fatal(r, e)
	}
	x := item("1.0001", "999999999999.999999", "0.0001", "exempt")
	x.DiscountType = "percent"
	r, e = Calculate(CalculationInput{Items: []Item{x}, DiscountType: "amount", Discount: "0"})
	if e != nil {
		t.Fatal(e)
	}
	if r.Items[0].UnitDiscount != "999999.999999999999" || r.Calculation.Total != "1000098999900.00" {
		t.Fatal(r)
	}
}
