package integration

import (
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func TestRateLimitConcurrentReservationExactCountAndWindow(t *testing.T) {
	s := fixture(t)
	var wg sync.WaitGroup
	var accepted atomic.Int32
	const limit = 24
	for i := 0; i < 64; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			e := s.Limit(ctx, "same-synthetic-ip", limit)
			if e == nil {
				accepted.Add(1)
			} else if ce, ok := e.(*core.Error); !ok || ce.Code != "rate_limited" {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	if accepted.Load() != limit {
		t.Fatal(accepted.Load())
	}
	var count int
	if e := s.DB.QueryRow(`SELECT attempts FROM auth_limits WHERE bucket=?`, security.Hash("same-synthetic-ip")).Scan(&count); e != nil || count != limit {
		t.Fatal(count, e)
	}
	if _, e := s.DB.Exec(`UPDATE auth_limits SET window_start=? WHERE bucket=?`, time.Now().Add(-16*time.Minute), security.Hash("same-synthetic-ip")); e != nil {
		t.Fatal(e)
	}
	if e := s.Limit(ctx, "same-synthetic-ip", limit); e != nil {
		t.Fatal(e)
	}
	if e := s.DB.QueryRow(`SELECT attempts FROM auth_limits WHERE bucket=?`, security.Hash("same-synthetic-ip")).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
}
func TestQuoteSearchUnicodeAcrossASCIINumberAndBusinessText(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	p := quoteInput()
	p.Customer.Name = "Cliente aprobación sintética"
	p.Reference = "Revisión Peña"
	p.Items[0].Description = "Conexión eléctrica"
	_ = makeQuote(t, s, a, "unicode-search", p)
	for _, query := range []string{"aprobación", "Revisión Peña", "Conexión", "COT-000001"} {
		v, e := s.List(ctx, a, quotes.Filter{Page: 1, PerPage: 25, Search: query})
		if e != nil || v.Total != 1 {
			t.Fatal(fmt.Sprint(query), v, e)
		}
	}
}
