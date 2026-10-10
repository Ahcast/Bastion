package suppliers

import (
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func TestSupplierIdentityAndLegacyText(t *testing.T) {
	for _, v := range []string{"  PROVEEDÓR   Ejemplo ", "proveedor ejemplo", "Proveedo\u0301r Ejemplo", "Ｐｒｏｖｅｅｄóｒ Ejemplo"} {
		if CanonicalName(v) != "proveedor ejemplo" {
			t.Fatal(v)
		}
	}
	if CanonicalName("Empresa S.A.") == CanonicalName("Empresa SA") {
		t.Fatal("punctuation merged")
	}
	for _, v := range []string{"08011990123456", "0801-1990-123456", "0801 1990 123456"} {
		n, e := CanonicalRTN(v)
		if e != nil || n != "08011990123456" {
			t.Fatal(n, e)
		}
	}
	for _, v := range []string{"123", "٠٨٠١١٩٩٠١٢٣٤٥٦", "0801199012345A"} {
		if _, e := CanonicalRTN(v); e == nil {
			t.Fatal(v)
		}
	}
	legacy, e := LegacySnapshot("Proveedor histórico sin ficha")
	if e != nil || legacy.ID != nil || legacy.Name != "Proveedor histórico sin ficha" {
		t.Fatal(legacy, e)
	}
	b, _ := json.Marshal(legacy)
	if !strings.Contains(string(b), `"supplier_id":null`) {
		t.Fatal(string(b))
	}
	if _, e = LegacySnapshot(" "); e == nil {
		t.Fatal("empty legacy name")
	}
}
func TestExactObservedDecimals(t *testing.T) {
	for v, want := range map[string]string{"": "", "000010.25": "10.250000", "0": "0.000000", "999999999999.999999": "999999999999.999999"} {
		got, e := Decimal(v, 18, 6)
		if e != nil || got != want {
			t.Fatal(v, got, e)
		}
	}
	for _, v := range []string{"1e3", "-0.1", "+1", "1,000.00", " 1", ".5", "1.", "1.0000001", "1000000000000", "NaN"} {
		if _, e := Decimal(v, 18, 6); e == nil {
			t.Fatal("invalid exact price", v)
		}
	}
	if v, e := Decimal("99999999999999.9999", 18, 4); e != nil || v != "99999999999999.9999" {
		t.Fatal(v, e)
	}
}
func TestSupplierValidationOwnsCollectionsAndRejectsUnsafeInputs(t *testing.T) {
	valid := Input{Name: "Proveedor", Reliability: "unknown", Active: true, Contacts: []Contact{{Name: "Contacto", Email: "CONTACT@example.test"}}, Products: []Product{{Name: "Producto", MOQ: "0010.25"}}, Categories: []string{"  Ferretería  "}, Brands: []string{"Marca"}}
	original := valid
	if e := Validate(&valid); e != nil {
		t.Fatal(e)
	}
	if original.Contacts[0].Email != "CONTACT@example.test" || original.Products[0].MOQ != "0010.25" || original.Categories[0] != "  Ferretería  " {
		t.Fatal("mutated caller slices")
	}
	if valid.Products[0].MOQ != "10.2500" {
		t.Fatal(valid.Products)
	}
	cases := []Input{{Name: "", Reliability: "unknown"}, {Name: strings.Repeat("界", 181), Reliability: "unknown"}, {Name: "A", Reliability: "invented"}, {Name: "A", Reliability: "unknown", Website: "javascript:alert(1)"}, {Name: "A", Reliability: "unknown", Website: "https://user:password@example.test"}, {Name: "A", Reliability: "unknown", Email: "a@example.test\n"}, {Name: "A", Reliability: "unknown", Categories: []string{"Marca", "Márça"}}, {Name: "A", Reliability: "unknown", Products: []Product{{Name: "P", MOQ: "1e2"}}}, {Name: "A", Reliability: "unknown", Contacts: make([]Contact, 21)}, {Name: "A", Reliability: "unknown", TeamID: "0"}}
	for i, p := range cases {
		if e := Validate(&p); e == nil {
			t.Fatal("invalid", i)
		}
	}
	rating := 6
	valid.Evaluation = &rating
	if Validate(&valid) == nil {
		t.Fatal("unbounded rating")
	}
}
func TestPriceEvidenceIsDatedAndNeverOfficial(t *testing.T) {
	now := time.Now().UTC()
	p := PriceInput{SourceURL: "https://example.test/item", SourceType: "web", ObservedAt: now.Add(-time.Hour), Price: "000.123456", Currency: "usd", Taxes: "unknown", Shipping: "unknown", Version: 1}
	if e := ValidatePrice(&p, now); e != nil || p.Price != "0.123456" || p.Currency != "USD" {
		t.Fatal(p, e)
	}
	for _, change := range []func(*PriceInput){func(p *PriceInput) { p.SourceURL = "" }, func(p *PriceInput) { p.SourceType = "supplier_quote" }, func(p *PriceInput) { p.ObservedAt = now.Add(time.Second) }, func(p *PriceInput) { p.Currency = "" }, func(p *PriceInput) { p.Price = "1.2345678" }, func(p *PriceInput) { p.Taxes = "yes" }, func(p *PriceInput) { p.Version = 0 }} {
		c := p
		change(&c)
		if ValidatePrice(&c, now) == nil {
			t.Fatal("unverified or invalid evidence accepted", c)
		}
	}
	p.Price = ""
	p.Currency = ""
	if e := ValidatePrice(&p, now); e != nil {
		t.Fatal("evidence without published price", e)
	}
}
