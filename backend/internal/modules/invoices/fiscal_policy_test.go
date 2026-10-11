package invoices

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"math"
	"strings"
	"testing"
)

func syntheticFiscal() FiscalAuthorization {
	return FiscalAuthorization{CAI: "AUTORIZACION-SINTETICA-NO-VALIDA", Prefix: "TEST-", Width: 8, RangeStart: 1, RangeEnd: 100, ValidFrom: "2026-10-01", ValidUntil: "2026-10-31", Currency: "HNL", Enabled: true}
}
func TestFiscalAllocationInclusiveExactAndImmutable(t *testing.T) {
	p := syntheticFiscal()
	for _, n := range []uint64{1, 100} {
		r, e := AllocateFiscalNumber(p, n, "2026-10-01", "2026-10-31", "HNL")
		if e != nil || r.NextValue != n+1 || r.Snapshot != p {
			t.Fatal(r, e)
		}
		if n == 1 && r.Number != "TEST-00000001" || n == 100 && r.Number != "TEST-00000100" {
			t.Fatal(r)
		}
		r.Snapshot.CAI = "changed"
		if p.CAI != syntheticFiscal().CAI {
			t.Fatal("snapshot aliases input")
		}
	}
	p.Currency = "USD"
	p.Width = 20
	p.RangeEnd = math.MaxUint64 - 1
	r, e := AllocateFiscalNumber(p, 9007199254740993, "2026-10-31", "2026-10-31", "USD")
	if e != nil || r.Number != "TEST-00009007199254740993" || r.NextValue != 9007199254740994 {
		t.Fatal(r, e)
	}
	r, e = AllocateFiscalNumber(p, p.RangeEnd, "2026-10-31", "2026-10-31", "USD")
	if e != nil || r.NextValue != math.MaxUint64 {
		t.Fatal(r, e)
	}
}
func TestFiscalAuthorizationRejectsInvalidConfiguration(t *testing.T) {
	cases := []func(*FiscalAuthorization){func(p *FiscalAuthorization) { p.CAI = "" }, func(p *FiscalAuthorization) { p.CAI = strings.Repeat("ñ", 41) }, func(p *FiscalAuthorization) { p.CAI = "CAI\nTEST" }, func(p *FiscalAuthorization) { p.Width = 0 }, func(p *FiscalAuthorization) { p.Width = 21 }, func(p *FiscalAuthorization) { p.Width = 2 }, func(p *FiscalAuthorization) { p.Prefix = "TEST \n" }, func(p *FiscalAuthorization) { p.Prefix = "ñ" }, func(p *FiscalAuthorization) { p.RangeStart = 0 }, func(p *FiscalAuthorization) { p.RangeStart = 101 }, func(p *FiscalAuthorization) { p.RangeEnd = math.MaxUint64 }, func(p *FiscalAuthorization) { p.ValidFrom = "2026-10-32" }, func(p *FiscalAuthorization) { p.ValidFrom = "2026-10-1" }, func(p *FiscalAuthorization) { p.ValidUntil = "2026-09-30" }, func(p *FiscalAuthorization) { p.Currency = "EUR" }}
	for i, change := range cases {
		p := syntheticFiscal()
		change(&p)
		if e := ValidateFiscalAuthorization(p); e != core.Invalid {
			t.Fatalf("case %d: %v", i, e)
		}
	}
}
func TestFiscalAllocationRejectsExhaustionExpiryBackdatingAndMismatch(t *testing.T) {
	for _, c := range []struct {
		next                        uint64
		invoice, business, currency string
	}{{0, "2026-10-11", "2026-10-11", "HNL"}, {101, "2026-10-11", "2026-10-11", "HNL"}, {1, "2026-10-31", "2026-11-01", "HNL"}, {1, "2026-09-30", "2026-10-11", "HNL"}, {1, "2026-10-12", "2026-10-11", "HNL"}, {1, "2026-10-11", "2026-10-11", "USD"}, {1, "2026-10-01", "2026-09-30", "HNL"}} {
		r, e := AllocateFiscalNumber(syntheticFiscal(), c.next, c.invoice, c.business, c.currency)
		if e != core.Conflict || r.Number != "" || r.NextValue != 0 {
			t.Fatal(c, r, e)
		}
	}
	p := syntheticFiscal()
	p.Enabled = false
	if _, e := AllocateFiscalNumber(p, 1, "2026-10-11", "2026-10-11", "HNL"); e != core.Conflict {
		t.Fatal(e)
	}
	for _, date := range []string{"2026-02-30", "2026-10-1", "0001-01-01"} {
		if _, e := AllocateFiscalNumber(syntheticFiscal(), 1, date, "2026-10-11", "HNL"); e != core.Invalid {
			t.Fatal(date, e)
		}
	}
}
