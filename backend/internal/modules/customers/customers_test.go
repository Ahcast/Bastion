package customers

import (
	"strings"
	"testing"
)

func TestCanonicalIdentity(t *testing.T) {
	for _, v := range []string{"  INSTITUCIÓN   Ejemplo ", "institucion ejemplo", "Institucio\u0301n\tEjemplo", "Ｉｎｓｔｉｔｕｃｉóｎ Ejemplo"} {
		if got := CanonicalName(v); got != "institucion ejemplo" {
			t.Fatalf("%q => %q", v, got)
		}
	}
	if CanonicalName("Cliente S.A.") == CanonicalName("Cliente SA") {
		t.Fatal("punctuation silently merged")
	}
	for _, v := range []string{"08011990123456", "0801-1990-123456", " 0801 1990 123456 "} {
		got, e := CanonicalRTN(v)
		if e != nil || got != "08011990123456" {
			t.Fatal(got, e)
		}
	}
	if v, e := CanonicalRTN(""); v != "" || e != nil {
		t.Fatal("empty RTN denied")
	}
	for _, v := range []string{"123", "0801199012345A", "٠٨٠١١٩٩٠١٢٣٤٥٦", "08011990123456\nX"} {
		if _, e := CanonicalRTN(v); e == nil {
			t.Fatal("invalid RTN", v)
		}
	}
}
func TestCustomerInputValidation(t *testing.T) {
	valid := Input{Name: "Cliente", InstitutionType: "public", Active: true, Contacts: []Contact{{Name: "Contacto", Email: "contact@example.test"}}, Addresses: []Address{{Label: "Principal", Address: "Dirección sintética"}}}
	if e := Validate(&valid); e != nil {
		t.Fatal(e)
	}
	cases := []Input{{Name: " ", InstitutionType: "other"}, {Name: strings.Repeat("界", 181), InstitutionType: "other"}, {Name: "A", InstitutionType: "invented"}, {Name: "A", InstitutionType: "other", Email: "Nombre <a@example.test>"}, {Name: "A", InstitutionType: "other", Email: "a@example.test\n"}, {Name: "A\x00", InstitutionType: "other"}, {Name: "A", InstitutionType: "other", TeamID: "0"}, {Name: "A", InstitutionType: "other", Contacts: []Contact{{Name: ""}}}, {Name: "A", InstitutionType: "other", Addresses: []Address{{Label: "Casa", Address: ""}}}, {Name: "A", InstitutionType: "other", Contacts: make([]Contact, 21)}}
	for i, p := range cases {
		if e := Validate(&p); e == nil {
			t.Fatal("invalid case accepted", i)
		}
	}
}
