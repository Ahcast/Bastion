package integration

import (
	"strings"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func fxObservationInput() quotes.FXObservationInput {
	return quotes.FXObservationInput{
		RateKind:          "tcr",
		Rate:              "26.123456789012",
		ValidFrom:         "2026-10-10T00:00:00-06:00",
		ValidTo:           "",
		Source:            "bch",
		SourceReference:   "synthetic:bch:tcr:2026-10-10",
		SourcePublishedAt: "2026-10-09T18:00:00-06:00",
		EvidenceHash:      strings.Repeat("a", 64),
		SupersedesID:      "",
		Reason:            "",
	}
}

func TestQuoteFXObservationLedgerIsGlobalExactAndIdempotent(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "self")
	p := fxObservationInput()
	if _, e := s.RecordFXObservation(ctx, a, "fx-denied-self-scope", p); e != core.Forbidden {
		t.Fatal("global FX ledger accepted self scope", e)
	}

	grantQuote(t, s.Core, a, "all")
	v, e := s.RecordFXObservation(ctx, a, "fx-first-observation", p)
	id := customerResult(t, v, e)["id"]
	if id == "" {
		t.Fatal("missing observation id")
	}
	v, e = s.RecordFXObservation(ctx, a, "fx-first-observation", p)
	if customerResult(t, v, e)["id"] != id {
		t.Fatal("idempotent replay changed observation")
	}

	changed := p
	changed.Rate = "26.223456789012"
	if _, e = s.RecordFXObservation(ctx, a, "fx-first-observation", changed); e != core.Conflict {
		t.Fatal("same key with changed FX payload must conflict", e)
	}

	o, e := s.GetFXObservation(ctx, a, id)
	if e != nil || o.BaseCurrency != "USD" || o.QuoteCurrency != "HNL" || o.Rate != "26.123456789012" || o.RateKind != "tcr" || o.Source != "bch" || o.Superseded {
		t.Fatal(o, e)
	}
	if o.ValidFrom.UTC().Format("2006-01-02T15:04:05Z") != "2026-10-10T06:00:00Z" {
		t.Fatal("valid_from was not normalized to UTC", o.ValidFrom)
	}

	page, e := s.ListFXObservations(ctx, a, quotes.FXFilter{RateKind: " TCR ", Source: " BCH ", Page: 1, PerPage: 25})
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != id {
		t.Fatal(page, e)
	}
}

func TestQuoteFXCorrectionsAreAppendOnlyAndReplaySafe(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	p := fxObservationInput()
	v, e := s.RecordFXObservation(ctx, a, "fx-original-rate", p)
	original := customerResult(t, v, e)["id"]

	correction := p
	correction.Rate = "26.200000000001"
	correction.SupersedesID = original
	correction.Reason = "Corrección sintética de la observación"
	v, e = s.RecordFXObservation(ctx, a, "fx-correction-rate", correction)
	corrected := customerResult(t, v, e)["id"]
	if corrected == original {
		t.Fatal("correction overwrote original observation")
	}
	v, e = s.RecordFXObservation(ctx, a, "fx-correction-rate", correction)
	if customerResult(t, v, e)["id"] != corrected {
		t.Fatal("correction replay changed id")
	}

	old, e := s.GetFXObservation(ctx, a, original)
	if e != nil || !old.Superseded || old.Rate != "26.123456789012" {
		t.Fatal(old, e)
	}
	newer, e := s.GetFXObservation(ctx, a, corrected)
	if e != nil || newer.SupersedesID != original || newer.Rate != "26.200000000001" {
		t.Fatal(newer, e)
	}

	second := correction
	second.Rate = "26.300000000001"
	second.Reason = "Segundo intento sintético"
	if _, e = s.RecordFXObservation(ctx, a, "fx-second-correction", second); e != core.Conflict {
		t.Fatal("same observation accepted two direct corrections", e)
	}
}

func TestQuoteFXValidationAndRollback(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")

	manual := fxObservationInput()
	manual.Source = "manual"
	manual.RateKind = "manual"
	manual.Reason = ""
	if _, e := s.RecordFXObservation(ctx, a, "fx-manual-no-reason", manual); e != core.Invalid {
		t.Fatal("manual observation without reason accepted", e)
	}
	badWindow := fxObservationInput()
	badWindow.ValidTo = badWindow.ValidFrom
	if _, e := s.RecordFXObservation(ctx, a, "fx-invalid-window", badWindow); e != core.Invalid {
		t.Fatal("invalid validity window accepted", e)
	}
	badRate := fxObservationInput()
	badRate.Rate = "26.1234567890123"
	if _, e := s.RecordFXObservation(ctx, a, "fx-invalid-rate", badRate); e != core.Invalid {
		t.Fatal("rate exceeding exact scale accepted", e)
	}

	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_fx_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic fx audit outage'`); e != nil {
		t.Fatal(e)
	}
	p := fxObservationInput()
	p.ValidFrom = "2026-10-11T00:00:00-06:00"
	p.SourceReference = "synthetic:bch:tcr:2026-10-11"
	if _, e := s.RecordFXObservation(ctx, a, "fx-rollback-observation", p); e == nil {
		t.Fatal("audit failure was swallowed")
	}
	var n int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quote_fx_observations`).Scan(&n); e != nil || n != 0 {
		t.Fatal("FX observation escaped rollback", n, e)
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM idempotency_records WHERE operation='quotes.fx_observation' AND request_key='fx-rollback-observation'`).Scan(&n); e != nil || n != 0 {
		t.Fatal("FX idempotency record escaped rollback", n, e)
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER test_fx_audit`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.RecordFXObservation(ctx, a, "fx-rollback-observation", p); e != nil {
		t.Fatal("retry after rolled-back failure did not recover", e)
	}
}
