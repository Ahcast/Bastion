package suppliers

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

func TestCompleteSupplierDTORejectsMissingNullAndProtectedFields(t *testing.T) {
	p := Input{Name: "Proveedor", Reliability: "unknown", Active: true, Contacts: []Contact{}, Products: []Product{}, Categories: []string{}, Brands: []string{}}
	b, _ := json.Marshal(p)
	var fields map[string]any
	_ = json.Unmarshal(b, &fields)
	decode := func(v any) error {
		b, _ := json.Marshal(v)
		r := httptest.NewRequest("PATCH", "/suppliers/1", bytes.NewReader(b))
		r.Header.Set("Content-Type", "application/json")
		var out Input
		return decodeInput(r, &out)
	}
	if e := decode(fields); e != nil {
		t.Fatal("null evaluations must be allowed", e)
	}
	for name := range fields {
		old := fields[name]
		delete(fields, name)
		if decode(fields) == nil {
			t.Fatal("missing", name)
		}
		fields[name] = nil
		if name != "evaluation" && name != "response_hours" && decode(fields) == nil {
			t.Fatal("null", name)
		}
		fields[name] = old
	}
	fields["created_by"] = "99"
	if decode(fields) == nil {
		t.Fatal("protected owner")
	}
	delete(fields, "created_by")
	fields["version"] = float64(1)
	if decode(fields) == nil {
		t.Fatal("numeric version")
	}
}
