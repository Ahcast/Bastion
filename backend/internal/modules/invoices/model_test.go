package invoices

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestDraftTermsAndStrictProtectedFields(t *testing.T) {
	p := Input{Customer: CustomerInput{Name: "Cliente sintético"}, InvoiceDate: "2026-10-11", PaymentTerm: "contado", Currency: "HNL", DiscountType: "amount", Discount: "0", AdditionalExpenses: "0"}
	for _, x := range mixedInvoice("HNL").Items {
		p.Items = append(p.Items, ItemInput{Item: x})
	}
	p.Exoneration = "Exoneración sintética"
	if _, e := Validate(p); e != nil {
		t.Fatal(e)
	}
	days := uint16(30)
	p.CreditDays = &days
	if _, e := Validate(p); e != core.Invalid {
		t.Fatal(e)
	}
	p.PaymentTerm = "credito"
	if _, e := Validate(p); e != nil {
		t.Fatal(e)
	}
	days = 0
	if _, e := Validate(p); e != core.Invalid {
		t.Fatal(e)
	}
	days = 30
	p.InvoiceDate = "2026-02-30"
	if _, e := Validate(p); e != core.Invalid {
		t.Fatal(e)
	}
	p.InvoiceDate = "2026-10-11"
	p.Items[0].ID = "1"
	p.Items[1].ID = "1"
	if _, e := Validate(p); e != core.Invalid {
		t.Fatal(e)
	}
	for _, body := range []string{`{}`, `{"currency":"HNL","items":[],"general_discount_type":"amount","general_discount":"0","exoneration_reference":"","additional_expenses":"0","invoice_number":"001"}`} {
		r := httptest.NewRequest("POST", "/", strings.NewReader(body))
		var p CalculationInput
		if e := decode(r, &p, false); e != core.Invalid {
			t.Fatal(e)
		}
	}
}
