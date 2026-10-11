package invoices

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"testing"
)

func TestCommercialStatusPaymentAndCalendarPrecedence(t *testing.T) {
	due := "2026-10-11"
	for _, c := range []struct{ paid, balance, today, want string }{{"0.00", "115.00", "2026-10-11", "pending"}, {"15.00", "100.00", "2026-10-11", "partially_paid"}, {"15.00", "100.00", "2026-10-12", "overdue"}, {"115.00", "0.00", "2026-10-12", "paid"}, {"0.00", "0.00", "2026-10-12", "paid"}, {"9007199254740993.00", "0.01", "2026-10-11", "partially_paid"}} {
		got, e := CommercialStatus("issued", &due, PaymentProjection{Currency: "HNL", PaidTotal: c.paid, Balance: c.balance}, c.today)
		if e != nil || got == nil || *got != c.want {
			t.Fatal(c, got, e)
		}
	}
	if got, e := CommercialStatus("draft", nil, PaymentProjection{}, "2026-10-11"); e != nil || got != nil {
		t.Fatal(got, e)
	}
	if _, e := CommercialStatus("issued", nil, PaymentProjection{Currency: "HNL"}, "2026-10-11"); e != core.Conflict {
		t.Fatal(e)
	}
}
