package integration

import (
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
)

func quoteInvoiceFixture(t *testing.T) (*quotes.Service, *invoices.Service, *purchaseorders.Service, core.Actor) {
	t.Helper()
	q, po, a := conversionFixture(t)
	inv := &invoices.Service{Core: q.Core}
	q.Invoices = inv
	grantInvoice(t, q.Core, a, "all")
	return q, inv, po, a
}
func quoteInvoiceInput() quotes.InvoiceConversionInput {
	return quotes.InvoiceConversionInput{Version: 2, InvoiceDate: invoiceToday(), PaymentTerm: "contado", AdditionalExpenses: "3.50", PublicNotes: "Nota pública explícita", Reason: "Aceptación verificada"}
}
func TestQuoteInvoiceConversionHistoricalMixedFinancialsAndLinks(t *testing.T) {
	q, inv, _, a := quoteInvoiceFixture(t)
	p := quoteInput()
	grantCustomer(t, q.Core, a, "all")
	cid := makeCustomer(t, q.Customers.(*customers.Service), a, "invoice-source-customer", customerInput("Cliente histórico de factura"))
	p.Customer.ID = cid
	p.Currency = "USD"
	p.Exoneration = "Exoneración sintética"
	p.DiscountType = "percent"
	p.Discount = "5"
	p.Items = nil
	for _, tax := range []string{"isv15", "isv18", "exempt", "exonerated"} {
		p.Items = append(p.Items, quotes.Item{Description: tax, Unit: "u", Quantity: "2", Price: "100", Cost: "30.005", DiscountType: "percent", Discount: "10", Tax: tax})
	}
	id := acceptedConversionQuote(t, q, a, "quote-invoice-mixed", p)
	before, e := q.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = q.Core.DB.Exec(`UPDATE customers SET name='Nombre posterior',is_active=FALSE,version=version+1 WHERE id=?`, cid); e != nil {
		t.Fatal(e)
	}
	if _, e = q.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission LIKE 'clientes.%'`); e != nil {
		t.Fatal(e)
	}
	meta := quoteInvoiceInput()
	days := uint16(30)
	meta.PaymentTerm = "credito"
	meta.CreditDays = &days
	v, e := q.ConvertToInvoice(ctx, a, id, "quote-invoice-convert", meta)
	result := customerResult(t, v, e)
	o, e := inv.Get(ctx, a, result["invoice_id"])
	if e != nil {
		t.Fatal(e)
	}
	if o.Status != "draft" || o.Number != nil || o.SAR != nil || o.PaidTotal != "0.00" || o.Balance != before.Calculation.Total || o.SourceQuoteID == nil || *o.SourceQuoteID != id || o.SourceQuoteVersion == nil || *o.SourceQuoteVersion != 2 || o.Customer.Name != before.Customer.Name || o.CustomerVersion != before.CustomerVersion || o.Currency != "USD" || o.Calculation.ProductsInvestment != "240.04" || o.Calculation.Profit != "440.46" || o.Calculation.Total != "740.43" || o.PaymentTerm != "credito" || *o.CreditDays != 30 || o.PublicNotes != meta.PublicNotes || len(o.Items) != 4 {
		t.Fatal(o, before)
	}
	for i, x := range o.Items {
		source := before.Items[i]
		if x.CalculatedItem != source {
			t.Fatal(i, x, source)
		}
	}
	after, e := q.Get(ctx, a, id)
	if e != nil || after.Status != "converted" || after.Version != 3 || after.InvoiceID == nil || *after.InvoiceID != o.ID || after.PurchaseOrderID != nil {
		t.Fatal(after, e)
	}
	edit := editInvoice(o)
	edit.PublicNotes = "Notas revisadas"
	if _, e = inv.Save(ctx, a, o.ID, "derived-draft-edit", edit); e != nil {
		t.Fatal(e)
	}
	o, e = inv.Get(ctx, a, o.ID)
	if e != nil || *o.SourceQuoteID != id || *o.SourceQuoteVersion != 2 || o.Version != 2 {
		t.Fatal(o, e)
	}
	v, e = q.ConvertToInvoice(ctx, a, id, "quote-invoice-convert", meta)
	if customerResult(t, v, e)["invoice_version"] != "1" {
		t.Fatal(v)
	}
	if _, e = q.ConvertToPurchaseOrder(ctx, a, id, "quote-already-invoice", conversionInput()); e != core.Conflict {
		t.Fatal(e)
	}
	cloned, e := q.Clone(ctx, a, id, "clone-derived-source", 3)
	clone := customerResult(t, cloned, e)
	copy, e := q.Get(ctx, a, clone["id"])
	if e != nil || copy.InvoiceID != nil || copy.PurchaseOrderID != nil {
		t.Fatal(copy, e)
	}
	var counters int
	if e = q.Core.DB.QueryRow(`SELECT COUNT(*) FROM document_counters WHERE name='invoice'`).Scan(&counters); e != nil || counters != 0 {
		t.Fatal(counters, e)
	}
}
func TestQuoteInvoiceConversionConcurrentReplayAndCompetingDestinations(t *testing.T) {
	q, inv, po, a := quoteInvoiceFixture(t)
	id := acceptedConversionQuote(t, q, a, "invoice-concurrent-source", quoteInput())
	meta := quoteInvoiceInput()
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, e := q.ConvertToInvoice(ctx, a, id, "invoice-replay-twelve", meta)
			errs <- e
		}()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	ledger, e := inv.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25})
	if e != nil || ledger.Total != 1 {
		t.Fatal(ledger, e)
	}
	id = acceptedConversionQuote(t, q, a, "invoice-destination-race", quoteInput())
	other := customerActor(t, q.Core, "all")
	grantQuote(t, q.Core, other, "all")
	grantInvoice(t, q.Core, other, "all")
	grantPurchaseOrders(t, q.Core, other, "all")
	errs = make(chan error, 2)
	wg.Add(2)
	go func() {
		defer wg.Done()
		_, e := q.ConvertToInvoice(ctx, a, id, "race-target-invoice", meta)
		errs <- e
	}()
	go func() {
		defer wg.Done()
		_, e := q.ConvertToPurchaseOrder(ctx, other, id, "race-target-order", conversionInput())
		errs <- e
	}()
	wg.Wait()
	close(errs)
	ok, conflict := 0, 0
	for e := range errs {
		if e == nil {
			ok++
		} else if e == core.Conflict {
			conflict++
		} else {
			t.Fatal(e)
		}
	}
	if ok != 1 || conflict != 1 {
		t.Fatal(ok, conflict)
	}
	source, e := q.Get(ctx, a, id)
	if e != nil || (source.InvoiceID == nil) == (source.PurchaseOrderID == nil) {
		t.Fatal(source, e)
	}
	_ = po
}
func TestQuoteInvoiceConversionRollbackScopesAndStrictHTTP(t *testing.T) {
	q, inv, _, a := quoteInvoiceFixture(t)
	id := acceptedConversionQuote(t, q, a, "invoice-http-source", quoteInput())
	meta := quoteInvoiceInput()
	if _, e := q.Core.DB.Exec(`CREATE TRIGGER quote_invoice_rollback BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := q.ConvertToInvoice(ctx, a, id, "invoice-rollback-intent", meta); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e := q.Core.DB.Exec(`DROP TRIGGER quote_invoice_rollback`); e != nil {
		t.Fatal(e)
	}
	source, e := q.Get(ctx, a, id)
	if e != nil || source.Status != "accepted" || source.InvoiceID != nil || source.Version != 2 {
		t.Fatal(source, e)
	}
	page, e := inv.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25})
	if e != nil || page.Total != 0 {
		t.Fatal(page, e)
	}
	other := customerActor(t, q.Core, "self")
	grantQuote(t, q.Core, other, "self")
	grantInvoice(t, q.Core, other, "self")
	if _, e = q.ConvertToInvoice(ctx, other, id, "invoice-self-forbidden", meta); e != core.Missing {
		t.Fatal(e)
	}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e = r.Register(q.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	if e = r.Register(inv.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(q.Core, r, nil, logger)
	token, csrf, e := q.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	raw, _ := json.Marshal(meta)
	var body map[string]any
	_ = json.Unmarshal(raw, &body)
	path := "/api/v1/quotes/" + id + "/convert-to-invoice"
	for name := range body {
		copy := map[string]any{}
		for k, v := range body {
			copy[k] = v
		}
		delete(copy, name)
		status, out := purchaseOrderHTTP(t, h, "POST", path, copy, cookie, csrf, q.Core.Config.Origin, "invoice-missing-"+name)
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["items"] = []any{}
	status, out := purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, q.Core.Config.Origin, "invoice-forged-items")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "items")
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, "", q.Core.Config.Origin, "invoice-no-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, q.Core.Config.Origin, "invoice-http-valid")
	if status != 200 || out["invoice_version"] != "1" {
		t.Fatal(status, out)
	}
	if _, e = q.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='facturas.create'`); e != nil {
		t.Fatal(e)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, q.Core.Config.Origin, "invoice-http-valid")
	if status != 403 {
		t.Fatal(status, out)
	}
	q.Invoices = nil
	if _, e = q.ConvertToInvoice(ctx, a, id, "disabled-invoice-target", meta); e != core.Missing {
		t.Fatal(e)
	}
}
func TestQuoteInvoiceConversionInvalidStateVersionAndCalculation(t *testing.T) {
	q, _, _, a := quoteInvoiceFixture(t)
	meta := quoteInvoiceInput()
	draft := makeQuote(t, q, a, "invoice-draft-source", quoteInput())
	meta.Version = 1
	if _, e := q.ConvertToInvoice(ctx, a, draft, "draft-source-rejected", meta); e != core.Conflict {
		t.Fatal(e)
	}
	id := acceptedConversionQuote(t, q, a, "invoice-invalid-source", quoteInput())
	if _, e := q.ConvertToInvoice(ctx, a, id, "source-version-stale", meta); e != core.Conflict {
		t.Fatal(e)
	}
	meta.Version = 2
	for i, mutate := range []func(*quotes.InvoiceConversionInput){func(p *quotes.InvoiceConversionInput) { p.PaymentTerm = "credito" }, func(p *quotes.InvoiceConversionInput) { p.InvoiceDate = "2026-02-30" }, func(p *quotes.InvoiceConversionInput) { p.AdditionalExpenses = "-1" }, func(p *quotes.InvoiceConversionInput) { p.Reason = "" }} {
		p := meta
		mutate(&p)
		if _, e := q.ConvertToInvoice(ctx, a, id, fmt.Sprint("invoice-invalid-meta-", i), p); e != core.Invalid {
			t.Fatal(p, e)
		}
	}
	if _, e := q.Core.DB.Exec(`UPDATE quotes SET total=total+0.01 WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e := q.ConvertToInvoice(ctx, a, id, "invoice-tampered-source", meta); e != core.Conflict {
		t.Fatal(e)
	}
}
