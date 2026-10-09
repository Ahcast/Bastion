package customers

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

func TestCompleteCustomerDTORejectsMissingNullAndProtectedFields(t *testing.T) {
	p := Input{Name: "Cliente", InstitutionType: "other", Active: true, Contacts: []Contact{}, Addresses: []Address{}}
	b, _ := json.Marshal(p)
	var fields map[string]any
	_ = json.Unmarshal(b, &fields)
	decode := func(v any) error {
		b, _ := json.Marshal(v)
		r := httptest.NewRequest("PATCH", "/customers/1", bytes.NewReader(b))
		r.Header.Set("Content-Type", "application/json")
		var out Input
		return decodeInput(r, &out)
	}
	if e := decode(fields); e != nil {
		t.Fatal("complete DTO rejected", e)
	}
	for _, name := range []string{"name", "is_active", "version", "contacts", "addresses", "rtn"} {
		original := fields[name]
		delete(fields, name)
		if e := decode(fields); e == nil {
			t.Fatal("missing field", name)
		}
		fields[name] = nil
		if e := decode(fields); e == nil {
			t.Fatal("null field", name)
		}
		fields[name] = original
	}
	fields["created_by"] = "99"
	if e := decode(fields); e == nil {
		t.Fatal("protected field")
	}
	delete(fields, "created_by")
	fields["version"] = float64(1)
	if e := decode(fields); e == nil {
		t.Fatal("numeric version")
	}
}
