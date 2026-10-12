package reports

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"net/http/httptest"
	"testing"
)

func TestStrictFilters(t *testing.T) {
	for _, x := range []struct {
		kind, url string
		home      bool
	}{{"orders", "?page=0", false}, {"orders", "?per_page=101", false}, {"orders", "?date_from=2026-02-30", false}, {"orders", "?date_from=2026-10-13&date_to=2026-10-12", false}, {"orders", "?currency=EUR", false}, {"orders", "?status=cancelled", false}, {"invoices", "?status=cancelled", false}, {"expenses", "?status=issued", false}, {"home", "?status=issued", true}, {"home", "?page=1", true}, {"orders", "?page=1&page=2", false}, {"orders", "?unknown=1", false}} {
		f, e := filter(httptest.NewRequest("GET", "/"+x.url, nil), x.home)
		if e == nil {
			e = validate(x.kind, f)
		}
		if e != core.Invalid {
			t.Fatal(x, e)
		}
	}
}
func TestProjectionPreservesOwnerFinancials(t *testing.T) {
	p, e := source("orders")
	if e != nil {
		t.Fatal(e)
	}
	for _, c := range p.columns {
		if c.name == "profit" && c.expression != "r.profit" {
			t.Fatal(c)
		}
	}
	f, e := filter(httptest.NewRequest("GET", "/?date_from=2026-10-12&date_to=2026-10-12&currency=USD", nil), false)
	if e != nil || validate("orders", f) != nil || f.Page != 1 || f.PerPage != 25 {
		t.Fatal(f, e)
	}
}
