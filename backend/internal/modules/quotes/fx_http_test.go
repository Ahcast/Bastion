package quotes

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

func TestFXObservationDTOIsStrictAndComplete(t *testing.T) {
	base := map[string]any{
		"rate_kind":           "tcr",
		"rate":                "26.123456789012",
		"valid_from":          "2026-10-10T00:00:00-06:00",
		"valid_to":            "",
		"source":              "bch",
		"source_reference":    "synthetic:bch:tcr:2026-10-10",
		"source_published_at": "2026-10-09T18:00:00-06:00",
		"evidence_hash":       "",
		"supersedes_id":       "",
		"reason":              "",
	}
	names := []string{"rate_kind", "rate", "valid_from", "valid_to", "source", "source_reference", "source_published_at", "evidence_hash", "supersedes_id", "reason"}
	decode := func(m map[string]any) error {
		b, _ := json.Marshal(m)
		r := httptest.NewRequest("POST", "/api/v1/quotes/fx-observations", bytes.NewReader(b))
		r.Header.Set("Content-Type", "application/json")
		var input FXObservationInput
		return required(r, &input, names...)
	}
	if e := decode(base); e != nil {
		t.Fatal(e)
	}
	for _, name := range names {
		old := base[name]
		delete(base, name)
		if e := decode(base); e == nil {
			t.Fatal("missing field accepted", name)
		}
		base[name] = nil
		if e := decode(base); e == nil {
			t.Fatal("null field accepted", name)
		}
		base[name] = old
	}
	base["base_currency"] = "EUR"
	if e := decode(base); e == nil {
		t.Fatal("protected base_currency accepted")
	}
}

func TestNormalizeFXObservationDoesNotRoundRate(t *testing.T) {
	p := FXObservationInput{
		RateKind:        " TCR ",
		Rate:            "26.123456789012",
		ValidFrom:       "2026-10-10T00:00:00-06:00",
		Source:          " BCH ",
		SourceReference: " synthetic:bch:tcr:2026-10-10 ",
	}
	from, to, published, e := normalizeFXInput(&p)
	if e != nil {
		t.Fatal(e)
	}
	if p.Rate != "26.123456789012" || p.RateKind != "tcr" || p.Source != "bch" {
		t.Fatal(p)
	}
	if from.Format("2006-01-02T15:04:05Z07:00") != "2026-10-10T06:00:00Z" || to != nil || published != nil {
		t.Fatal(from, to, published)
	}
}
