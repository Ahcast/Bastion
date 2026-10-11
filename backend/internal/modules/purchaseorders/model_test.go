package purchaseorders

import (
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func validInput() Input {
	delivery := "2026-10-30"
	return Input{
		OrderNumber: " OC-123 ",
		Customer: CustomerInput{
			Name:    "Cliente sintético",
			RTN:     "08011999123456",
			Contact: "Compras",
			Email:   "compras@example.test",
		},
		Destination:          "",
		OrderDate:            "2026-10-10",
		RequiredDeliveryDate: &delivery,
		Currency:             "HNL",
		Comments:             "Prueba",
		CalculationInput: CalculationInput{
			Items:              []Item{orderItem("1", "10.00", "2.500000", "0", "isv15")},
			DiscountType:       "amount",
			Discount:           "0.00",
			AdditionalExpenses: "1.20",
		},
	}
}

func TestValidateNormalizesAggregateWithoutInventingDestinationRule(t *testing.T) {
	p := validInput()
	r, e := Validate(&p)
	if e != nil {
		t.Fatal(e)
	}
	if p.OrderNumber != "OC-123" || p.Destination != "" || p.Discount != "0" || p.AdditionalExpenses != "1.20" {
		t.Fatal(p)
	}
	if len(p.Items) != 1 || p.Items[0].UnitSalePrice != "10" || p.Items[0].UnitCost != "2.5" {
		t.Fatal(p.Items)
	}
	if r.Calculation.Total != "11.50" || r.Calculation.ProductsInvestment != "2.50" || r.Calculation.InvestmentTotal != "3.70" {
		t.Fatal(r.Calculation)
	}
}

func TestValidateRejectsInvalidIdentityDatesAndText(t *testing.T) {
	cases := []func(*Input){
		func(p *Input) { p.OrderNumber = "   " },
		func(p *Input) { p.Customer.Name = "" },
		func(p *Input) { p.Customer.RTN = "123" },
		func(p *Input) { p.Customer.Email = "Name <compras@example.test>" },
		func(p *Input) { p.Customer.ID = "not-an-id" },
		func(p *Input) { p.OrderDate = "2026-02-30" },
		func(p *Input) { x := ""; p.RequiredDeliveryDate = &x },
		func(p *Input) { p.Currency = "EUR" },
		func(p *Input) { p.TeamID = "bad" },
		func(p *Input) { p.Items[0].SupplierID = "bad" },
	}
	for i, mutate := range cases {
		p := validInput()
		mutate(&p)
		if _, e := Validate(&p); e != core.Invalid {
			t.Fatalf("case %d: %v", i, e)
		}
	}
}

func TestValidateAllowsNullableDeliveryDateAndManualSupplier(t *testing.T) {
	p := validInput()
	p.RequiredDeliveryDate = nil
	p.Items[0].SupplierName = "Proveedor manual"
	p.Items[0].Tracking = "Pendiente de guía"
	p.Items[0].Comments = "Compra aún no emitida"
	if _, e := Validate(&p); e != nil {
		t.Fatal(e)
	}
	if p.Items[0].SupplierName != "Proveedor manual" {
		t.Fatal(p.Items[0])
	}
}
