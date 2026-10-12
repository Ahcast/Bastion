package expenses

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"testing"
)

func TestExpenseCalculatorLegacyRoundingAndBoundaries(t *testing.T) {
	for _, x := range []struct{ a, b, tax, total string }{{"100", "20", "15.00", "135.00"}, {"0.03", "0", "0.00", "0.03"}, {"0.10", "0", "0.02", "0.12"}, {"0", "0", "0.00", "0.00"}, {"8000000000000000.01", "0", "1200000000000000.00", "9200000000000000.01"}, {"0", "9999999999999999.99", "0.00", "9999999999999999.99"}} {
		for _, currency := range []string{"HNL", "USD"} {
			v, e := Calculate(CalculationInput{currency, x.a, x.b})
			if e != nil || v.Tax != x.tax || v.Total != x.total || v.Rate != "0.1500" || v.Currency != currency {
				t.Fatal(x, v, e)
			}
		}
	}
}
func TestExpenseCalculatorRejectsInvalidAndOverflow(t *testing.T) {
	for _, x := range []CalculationInput{{"EUR", "1", "0"}, {"HNL", "-1", "0"}, {"HNL", "1e2", "0"}, {"HNL", "01", "0"}, {"HNL", "1.001", "0"}, {"HNL", "9999999999999999.99", "0"}, {"HNL", "0", "10000000000000000"}, {"HNL", "NaN", "0"}, {"HNL", "", "0"}} {
		if _, e := Calculate(x); e != core.Invalid {
			t.Fatal(x, e)
		}
	}
}
func TestExpenseInputDateCategoryAndText(t *testing.T) {
	p := Input{Date: "2026-10-12", Supplier: Supplier{Name: "Proveedor libre"}, Category: "other", CalculationInput: CalculationInput{"HNL", "10", "0"}}
	if _, e := Validate(p); e != nil {
		t.Fatal(e)
	}
	for _, x := range []Input{func() Input { v := p; v.Date = "2026-02-30"; return v }(), func() Input { v := p; v.Category = "arbitrary"; return v }(), func() Input { v := p; v.Supplier.Name = ""; return v }(), func() Input { v := p; v.TeamID = "-1"; return v }()} {
		if _, e := Validate(x); e != core.Invalid {
			t.Fatal(x, e)
		}
	}
}
