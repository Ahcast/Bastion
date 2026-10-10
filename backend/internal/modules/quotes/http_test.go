package quotes

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

func TestQuoteDTORejectsMissingNestedNullAndProtectedFields(t *testing.T) {
	p := Input{Customer: CustomerInput{Name: "Cliente"}, Date: "2026-10-10", ValidityDays: 30, Currency: "HNL", CalculationInput: CalculationInput{Items: []Item{}, DiscountType: "amount", Discount: "0"}}
	b, _ := json.Marshal(p)
	var base map[string]any
	_ = json.Unmarshal(b, &base)
	names := []string{"customer", "reference", "date", "validity_days", "currency", "terms", "private_notes", "author_contact", "exoneration_reference", "general_discount_type", "general_discount", "items", "version"}
	decode := func(m map[string]any) error {
		b, _ := json.Marshal(m)
		r := httptest.NewRequest("POST", "/quotes", bytes.NewReader(b))
		r.Header.Set("Content-Type", "application/json")
		var input Input
		return required(r, &input, names...)
	}
	if e := decode(base); e != nil {
		t.Fatal(e)
	}
	for _, n := range names {
		old := base[n]
		delete(base, n)
		if e := decode(base); e == nil {
			t.Fatal("missing", n)
		}
		base[n] = nil
		if e := decode(base); e == nil {
			t.Fatal("null", n)
		}
		base[n] = old
	}
	c := base["customer"].(map[string]any)
	delete(c, "email")
	if e := decode(base); e == nil {
		t.Fatal("missing customer field")
	}
	c["email"] = ""
	for _, n := range []string{"created_by", "team_id", "number", "status", "source_process_id", "calculation"} {
		base[n] = "protected"
		if e := decode(base); e == nil {
			t.Fatal(n)
		}
		delete(base, n)
	}
	base["version"] = 0
	if e := decode(base); e == nil {
		t.Fatal("numeric version")
	}
}
