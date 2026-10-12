package integration

import (
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
)

func TestInvoiceCancellationPreservesFiscalEvidenceAndBlocksMutations(t *testing.T) {
	s, a, id, emission := invoiceEmissionFixture(t)
	if _, e := s.Emit(ctx, a, id, "cancel-emit", emission); e != nil {
		t.Fatal(e)
	}
	issued, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	var counter uint64
	if e = s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&counter); e != nil {
		t.Fatal(e)
	}
	p := invoices.CancelInput{Version: issued.Version, Reason: "Error sintético comprobado", OriginalAndCopyMarkedAndArchived: true}
	value, err := s.Cancel(ctx, a, id, "cancel-issued", p)
	first := customerResult(t, value, err)
	if first["status"] != "cancelled" || first["invoice_number"] != *issued.Number {
		t.Fatal(first)
	}
	value, err = s.Cancel(ctx, a, id, "cancel-issued", p)
	replay := customerResult(t, value, err)
	if replay["version"] != first["version"] || replay["cancelled_at"] != first["cancelled_at"] {
		t.Fatal(replay, first)
	}
	current, e := s.Get(ctx, a, id)
	if e != nil || current.Status != "cancelled" || current.CommercialStatus != nil || current.CancelledAt == nil || current.CancelReason == nil || *current.CancelReason != p.Reason || string(current.SAR) != string(issued.SAR) || string(current.Company) != string(issued.Company) || current.Balance != issued.Balance {
		t.Fatal(current, e)
	}
	var after uint64
	if e = s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&after); e != nil || counter != after {
		t.Fatal(counter, after, e)
	}
	doc, e := invoices.ProjectDocument(invoices.DocumentInvoice, current, nil, nil)
	if e != nil || doc.Label != "Factura anulada" {
		t.Fatal(doc, e)
	}
	if _, _, e = invoices.RenderDocumentPDF(doc); e != nil {
		t.Fatal(e)
	}
	if _, e = invoices.ProjectDocument(invoices.DocumentDeliveryNote, current, nil, nil); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.RegisterPayment(ctx, a, id, "cancel-payment", invoices.PaymentInput{Version: current.Version, Date: invoiceToday(), Method: "Efectivo", Currency: current.Currency, Amount: "1.00", Reason: "No permitido"}); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "cancel-edit", editInvoice(current)); e != core.Conflict {
		t.Fatal(e)
	}
	page, e := s.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25, Status: "cancelled"})
	if e != nil || page.Total != 1 || page.Data[0].ID != id {
		t.Fatal(page, e)
	}
}

func TestInvoiceCancellationRejectsPriorPaymentsAndGeneratedDocuments(t *testing.T) {
	s, a, id, emission := invoiceEmissionFixture(t)
	if _, e := s.Emit(ctx, a, id, "cancel-guard-emit", emission); e != nil {
		t.Fatal(e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := invoices.CancelInput{Version: o.Version, Reason: "Motivo", OriginalAndCopyMarkedAndArchived: true}
	if _, e = s.Cancel(ctx, a, id, "cancel-no-attestation", invoices.CancelInput{Version: o.Version, Reason: "Motivo"}); e != core.Invalid {
		t.Fatal(e)
	}
	if _, e = s.Cancel(ctx, a, id, "cancel-stale", invoices.CancelInput{Version: o.Version + 1, Reason: "Motivo", OriginalAndCopyMarkedAndArchived: true}); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.RegisterPayment(ctx, a, id, "cancel-guard-pay", invoices.PaymentInput{Version: o.Version, Date: invoiceToday(), Method: "Efectivo", Currency: o.Currency, Amount: "1.00", Reason: "Pago sintético"}); e != nil {
		t.Fatal(e)
	}
	p.Version++
	if _, e = s.Cancel(ctx, a, id, "cancel-with-payment", p); e != core.Conflict {
		t.Fatal(e)
	}
	var status string
	if e = s.Core.DB.QueryRow(`SELECT status FROM invoices WHERE id=?`, id).Scan(&status); e != nil || status != "issued" {
		t.Fatal(status, e)
	}
}
