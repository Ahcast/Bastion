package invoices

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"testing"
	"time"
)

func TestInvoiceBusinessDateUsesHondurasAcrossUTCBoundary(t *testing.T) {
	for _, c := range []struct{ instant, want string }{{"2026-10-12T05:59:59Z", "2026-10-11"}, {"2026-10-12T06:00:00Z", "2026-10-12"}, {"2027-01-01T00:01:00Z", "2026-12-31"}} {
		now, e := time.Parse(time.RFC3339, c.instant)
		if e != nil {
			t.Fatal(e)
		}
		got, e := invoiceBusinessDate(now)
		if e != nil || got != c.want {
			t.Fatal(c, got, e)
		}
	}
}
func TestInvoiceDueDateUsesCalendarDaysAndRejectsOverflow(t *testing.T) {
	days := uint16(2)
	o := Invoice{InvoiceDate: "2028-02-28", PaymentTerm: "credito", CreditDays: &days}
	if got, e := invoiceDueDate(o); e != nil || got != "2028-03-01" {
		t.Fatal(got, e)
	}
	o.PaymentTerm = "contado"
	o.CreditDays = nil
	if got, e := invoiceDueDate(o); e != nil || got != "2028-02-28" {
		t.Fatal(got, e)
	}
	o.PaymentTerm = "credito"
	o.CreditDays = &days
	o.InvoiceDate = "9999-12-31"
	if _, e := invoiceDueDate(o); e != core.Invalid {
		t.Fatal(e)
	}
	o.InvoiceDate = "2026-02-30"
	if _, e := invoiceDueDate(o); e != core.Invalid {
		t.Fatal(e)
	}
	o.InvoiceDate = "2026-10-11"
	o.CreditDays = nil
	if _, e := invoiceDueDate(o); e != core.Invalid {
		t.Fatal(e)
	}
}
