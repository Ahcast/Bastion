package integration

import (
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
	"time"
)

func orderInvoiceFixture(t *testing.T, inputs ...purchaseorders.Input) (*purchaseorders.Service, *invoices.Service, core.Actor, string, purchaseorders.InvoiceConversionInput) {
	t.Helper()
	po, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, po.Core, a, "all")
	inv := &invoices.Service{Core: po.Core, PurchaseOrders: po}
	po.Invoices = inv
	grantInvoice(t, po.Core, a, "all")
	id := ""
	if len(inputs) > 0 {
		v, e := po.Save(ctx, a, "", "order-custom-source", inputs[0])
		id = customerResult(t, v, e)["id"]
	} else {
		id = makeStateOrder(t, po, a)
	}
	if _, e := po.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	for _, to := range []string{"purchased", "shipped", "received", "delivered"} {
		stateChange(t, po, a, id, 1, to)
	}
	for _, to := range []string{"confirmed", "purchasing", "in_transit", "goods_received", "preparing_delivery", "delivered"} {
		stateChange(t, po, a, id, 0, to)
	}
	if e := po.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Empresa OC sintética","tax_id":"08011900123456","address":"Dirección sintética"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	fiscal := syntheticFiscalSettings()
	source, e := po.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	fiscal.Value.Currency = source.Currency
	today, _ := time.Parse("2006-01-02", invoiceToday())
	fiscal.Value.ValidFrom = today.AddDate(0, 0, -1).Format("2006-01-02")
	fiscal.Value.ValidUntil = today.AddDate(0, 0, 1).Format("2006-01-02")
	if _, e := inv.SaveFiscalSettings(ctx, a, "order-fiscal-settings", fiscal); e != nil {
		t.Fatal(e)
	}
	o, e := po.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	return po, inv, a, id, purchaseorders.InvoiceConversionInput{Version: o.Version, InvoiceDate: invoiceToday(), PaymentTerm: "contado", PublicNotes: "Nota pública de factura de OC", Reason: "Entrega verificada"}
}
func issueOrderInvoice(t *testing.T, po *purchaseorders.Service, inv *invoices.Service, a core.Actor, id string, p purchaseorders.InvoiceConversionInput) (string, invoices.EmitInput) {
	t.Helper()
	v, e := po.ConvertToInvoice(ctx, a, id, "order-invoice-convert", p)
	iid := customerResult(t, v, e)["invoice_id"]
	c, e := inv.EmissionContext(ctx, a, iid)
	if e != nil || c.SourceOrderVersion == nil {
		t.Fatal(c, e)
	}
	emit := invoices.EmitInput{Version: c.InvoiceVersion, FiscalVersion: c.FiscalVersion, CompanyVersion: c.CompanyVersion, SourceOrderVersion: c.SourceOrderVersion, Reason: "Factura y OC revisadas"}
	if _, e = inv.Emit(ctx, a, iid, "order-invoice-emit", emit); e != nil {
		t.Fatal(e)
	}
	return iid, emit
}
func TestOrderInvoiceBillingRoundTripCloseReopenAndPaymentInvariant(t *testing.T) {
	po, inv, a, id, p := orderInvoiceFixture(t)
	before, e := po.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	iid, emit := issueOrderInvoice(t, po, inv, a, id, p)
	o, e := po.Get(ctx, a, id)
	if e != nil || o.Status != "invoiced" || o.Version != before.Version+2 || o.InvoiceID == nil || *o.InvoiceID != iid || o.InvoiceNumber == nil || *o.InvoiceNumber != "TEST-00000001" {
		t.Fatal(o, e)
	}
	invoice, e := inv.Get(ctx, a, iid)
	if e != nil || invoice.SourceQuoteID != nil || invoice.SourcePurchaseOrderID == nil || *invoice.SourcePurchaseOrderID != id || invoice.SourcePurchaseOrderVersion == nil || *invoice.SourcePurchaseOrderVersion != before.Version || invoice.Calculation.Profit != before.Calculation.Profit || invoice.Customer != (invoices.CustomerInput{ID: before.Customer.ID, Name: before.Customer.Name, RTN: before.Customer.RTN, Contact: before.Customer.Contact, Phone: before.Customer.Phone, Email: before.Customer.Email, Address: before.Customer.Address}) {
		t.Fatal(invoice, e)
	}
	if _, e = po.CloseInvoice(ctx, a, id, "order-close-unpaid", purchaseorders.InvoiceClosureInput{Version: o.Version, InvoiceVersion: invoice.Version, Reason: "Cierre"}, false); e != core.Conflict {
		t.Fatal(e)
	}
	v, e := inv.RegisterPayment(ctx, a, iid, "order-invoice-pay-full", invoices.PaymentInput{Version: invoice.Version, Date: invoiceToday(), Method: "Transferencia", Currency: invoice.Currency, Amount: invoice.Calculation.Total, Reason: "Cobro completo sintético"})
	payment := paymentResult(t, v, e)
	close := purchaseorders.InvoiceClosureInput{Version: o.Version, InvoiceVersion: payment.Version, Reason: "Entrega y cobro completos"}

	if _, e = po.Core.DB.Exec(`CREATE TRIGGER order_close_rollback BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e = po.CloseInvoice(ctx, a, id, "order-close-confirmed", close, false); e == nil {
		t.Fatal("expected close rollback")
	}
	if _, e = po.Core.DB.Exec(`DROP TRIGGER order_close_rollback`); e != nil {
		t.Fatal(e)
	}
	check, e := po.Get(ctx, a, id)
	if e != nil || check.Status != "invoiced" || check.Version != o.Version {
		t.Fatal(check, e)
	}
	v, e = po.CloseInvoice(ctx, a, id, "order-close-confirmed", close, false)
	closed := customerResult(t, v, e)
	if closed["status"] != "closed" {
		t.Fatal(closed)
	}
	ledger, e := inv.Payments(ctx, a, iid, 1, 25)
	if e != nil || ledger.SourceOrderClosed == nil || !*ledger.SourceOrderClosed {
		t.Fatal(ledger, e)
	}
	void := invoices.VoidPaymentInput{Version: payment.Version, PaymentVersion: 1, Reason: "Corrección sintética"}
	if _, e = inv.VoidPayment(ctx, a, iid, payment.PaymentID, "order-payment-void", void); e != core.Conflict {
		t.Fatal(e)
	}
	o, e = po.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	v, e = po.CloseInvoice(ctx, a, id, "order-reopen-confirmed", purchaseorders.InvoiceClosureInput{Version: o.Version, InvoiceVersion: payment.Version, Reason: "Revisión de cobro"}, true)
	if customerResult(t, v, e)["status"] != "invoiced" {
		t.Fatal(v)
	}
	v, e = inv.VoidPayment(ctx, a, iid, payment.PaymentID, "order-payment-void", void)
	voided := paymentResult(t, v, e)
	if voided.PaidTotal != "0.00" || voided.Balance != invoice.Calculation.Total {
		t.Fatal(voided)
	}
	v, e = po.CloseInvoice(ctx, a, id, "order-close-confirmed", close, false)
	if customerResult(t, v, e)["status"] != "closed" {
		t.Fatal(v)
	}
	o, e = po.Get(ctx, a, id)
	if e != nil || o.Status != "invoiced" {
		t.Fatal(o, e)
	}
	if _, e = po.CloseInvoice(ctx, a, id, "order-close-unpaid-again", purchaseorders.InvoiceClosureInput{Version: o.Version, InvoiceVersion: voided.Version, Reason: "Cierre sin cobro"}, false); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = inv.Emit(ctx, a, iid, "order-invoice-emit", emit); e != nil {
		t.Fatal(e)
	}
	hist, e := po.StatusHistory(ctx, a, id, 1, 100)
	if e != nil {
		t.Fatal(e)
	}
	counts := map[string]int{}
	for _, row := range hist.Data {
		counts[row.ToStatus]++
	}
	if hist.Total != 13 || counts["closed"] != 1 || counts["invoiced"] != 2 {
		t.Fatal(hist)
	}
	if _, e = po.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='ordenes.convert'`); e != nil {
		t.Fatal(e)
	}
	if _, e = inv.Emit(ctx, a, iid, "order-invoice-emit", emit); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestOrderInvoiceBillingConversionReplayAndRollbackAllOwners(t *testing.T) {
	po, inv, a, id, p := orderInvoiceFixture(t)
	if _, e := po.Core.DB.Exec(`CREATE TRIGGER order_invoice_rollback BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := po.ConvertToInvoice(ctx, a, id, "order-convert-rollback", p); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e := po.Core.DB.Exec(`DROP TRIGGER order_invoice_rollback`); e != nil {
		t.Fatal(e)
	}
	source, e := po.Get(ctx, a, id)
	if e != nil || source.InvoiceID != nil || source.Version != p.Version {
		t.Fatal(source, e)
	}
	list, e := inv.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25})
	if e != nil || list.Total != 0 {
		t.Fatal(list, e)
	}
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, e := po.ConvertToInvoice(ctx, a, id, "order-convert-rollback", p)
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
	source, e = po.Get(ctx, a, id)
	if e != nil || source.InvoiceID == nil || source.Status != "delivered" || source.Version != p.Version+1 {
		t.Fatal(source, e)
	}
	iid := *source.InvoiceID
	invoice, e := inv.Get(ctx, a, iid)
	if e != nil || invoice.Status != "draft" || invoice.Number != nil {
		t.Fatal(invoice, e)
	}
	c, e := inv.EmissionContext(ctx, a, iid)
	if e != nil {
		t.Fatal(e)
	}
	emit := invoices.EmitInput{Version: c.InvoiceVersion, FiscalVersion: c.FiscalVersion, CompanyVersion: c.CompanyVersion, SourceOrderVersion: c.SourceOrderVersion, Reason: "Revisada"}
	if _, e = po.Core.DB.Exec(`CREATE TRIGGER order_issue_rollback BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e = inv.Emit(ctx, a, iid, "order-emission-rollback", emit); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e = po.Core.DB.Exec(`DROP TRIGGER order_issue_rollback`); e != nil {
		t.Fatal(e)
	}
	assertInvoiceNotIssued(t, inv, iid, 1)
	source, e = po.Get(ctx, a, id)
	if e != nil || source.Status != "delivered" || source.InvoiceNumber != nil || source.Version != p.Version+1 {
		t.Fatal(source, e)
	}
	if _, e = inv.Emit(ctx, a, iid, "order-emission-rollback", emit); e != nil {
		t.Fatal(e)
	}
}
func TestOrderInvoiceBillingRejectStateParityVersionsScopesAndReauth(t *testing.T) {
	po, inv, a, id, p := orderInvoiceFixture(t)
	other := customerActor(t, po.Core, "self")
	grantPurchaseOrders(t, po.Core, other, "self")
	grantInvoice(t, po.Core, other, "self")
	if _, e := po.ConvertToInvoice(ctx, other, id, "order-source-forbidden", p); e != core.Missing {
		t.Fatal(e)
	}
	stale := p
	stale.Version--
	if _, e := po.ConvertToInvoice(ctx, a, id, "order-source-stale", stale); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e := po.Core.DB.Exec(`UPDATE customer_purchase_order_items SET status='received' WHERE purchase_order_id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e := po.ConvertToInvoice(ctx, a, id, "order-source-unready", p); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e := po.Core.DB.Exec(`UPDATE customer_purchase_order_items SET status='delivered' WHERE purchase_order_id=?`, id); e != nil {
		t.Fatal(e)
	}
	v, e := po.ConvertToInvoice(ctx, a, id, "order-source-convert", p)
	iid := customerResult(t, v, e)["invoice_id"]
	o, e := inv.Get(ctx, a, iid)
	if e != nil {
		t.Fatal(e)
	}
	edit := editInvoice(o)
	edit.Items[0].Price = "101"
	if _, e = inv.Save(ctx, a, iid, "order-derived-price-edit", edit); e != nil {
		t.Fatal(e)
	}
	if _, e = inv.EmissionContext(ctx, a, iid); e != core.Conflict {
		t.Fatal(e)
	}
	edit.Version = 2
	edit.Items[0].Price = "100"
	if _, e = inv.Save(ctx, a, iid, "order-derived-price-restore", edit); e != nil {
		t.Fatal(e)
	}
	c, e := inv.EmissionContext(ctx, a, iid)
	if e != nil {
		t.Fatal(e)
	}
	bad := uint64(1)
	emit := invoices.EmitInput{Version: c.InvoiceVersion, FiscalVersion: c.FiscalVersion, CompanyVersion: c.CompanyVersion, SourceOrderVersion: &bad, Reason: "Revisada"}
	if _, e = inv.Emit(ctx, a, iid, "order-version-stale", emit); e != core.Conflict {
		t.Fatal(e)
	}
	emit.SourceOrderVersion = nil
	if _, e = inv.Emit(ctx, a, iid, "order-version-missing", emit); e != core.Invalid {
		t.Fatal(e)
	}
	emit.SourceOrderVersion = c.SourceOrderVersion
	if _, e = po.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = inv.Emit(ctx, a, iid, "order-no-reauth", emit); e == nil || e.Error() != "reauth_required" {
		t.Fatal(e)
	}
	assertInvoiceNotIssued(t, inv, iid, 1)
}
func TestOrderInvoiceBillingCloseVoidContentionKeepsClosedPaid(t *testing.T) {
	po, inv, a, id, p := orderInvoiceFixture(t)
	iid, _ := issueOrderInvoice(t, po, inv, a, id, p)
	invoice, e := inv.Get(ctx, a, iid)
	if e != nil {
		t.Fatal(e)
	}
	v, e := inv.RegisterPayment(ctx, a, iid, "order-race-pay-full", invoices.PaymentInput{Version: invoice.Version, Date: invoiceToday(), Method: "Efectivo", Currency: invoice.Currency, Amount: invoice.Calculation.Total, Reason: "Cobro"})
	payment := paymentResult(t, v, e)
	source, e := po.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	other := customerActor(t, po.Core, "all")
	grantPurchaseOrders(t, po.Core, other, "all")
	grantInvoice(t, po.Core, other, "all")
	var wg sync.WaitGroup
	errs := make(chan error, 2)
	wg.Add(2)
	go func() {
		defer wg.Done()
		_, e := po.CloseInvoice(ctx, a, id, "order-race-close", purchaseorders.InvoiceClosureInput{Version: source.Version, InvoiceVersion: payment.Version, Reason: "Cerrar"}, false)
		errs <- e
	}()
	go func() {
		defer wg.Done()
		_, e := inv.VoidPayment(ctx, other, iid, payment.PaymentID, "order-race-void", invoices.VoidPaymentInput{Version: payment.Version, PaymentVersion: 1, Reason: "Corregir"})
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
	source, e = po.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	invoice, e = inv.Get(ctx, a, iid)
	if e != nil || source.Status == "closed" && invoice.Balance != "0.00" || source.Status == "invoiced" && invoice.PaidTotal != "0.00" {
		t.Fatal(source, invoice, e)
	}
}
func TestOrderInvoiceBillingHTTPStrictCSRFAndSourceFlag(t *testing.T) {
	po, inv, a, id, p := orderInvoiceFixture(t)
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e := r.Register(po.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	if e := r.Register(inv.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(po.Core, r, nil, logger)
	token, csrf, e := po.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	raw, _ := json.Marshal(p)
	var body map[string]any
	_ = json.Unmarshal(raw, &body)
	path := "/api/v1/purchase-orders/" + id + "/convert-to-invoice"
	for name := range body {
		copy := map[string]any{}
		for k, v := range body {
			copy[k] = v
		}
		delete(copy, name)
		status, out := purchaseOrderHTTP(t, h, "POST", path, copy, cookie, csrf, po.Core.Config.Origin, "order-http-missing-"+name)
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["items"] = []any{}
	status, out := purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, po.Core.Config.Origin, "order-http-forged")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "items")
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, "", po.Core.Config.Origin, "order-http-no-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, po.Core.Config.Origin, "order-http-valid")
	if status != 200 || out["status"] != "delivered" || out["invoice_version"] != "1" {
		t.Fatal(status, out)
	}
	po.Invoices = nil
	if _, e = po.ConvertToInvoice(ctx, a, id, "order-target-disabled", p); e != core.Disabled {
		t.Fatal(e)
	}
	inv.PurchaseOrders = nil
	if _, e = inv.EmissionContext(ctx, a, out["invoice_id"].(string)); e != core.Disabled {
		t.Fatal(e)
	}
}

func TestOrderInvoiceBillingUSDAndZeroTotalWithoutFabricatedPayment(t *testing.T) {
	for _, zero := range []bool{false, true} {
		t.Run(fmt.Sprint(zero), func(t *testing.T) {
			input := purchaseOrderInput("", "")
			input.Currency = "USD"
			if zero {
				input.Items[0].UnitSalePrice = "0"
				input.Items[0].UnitCost = "0"
				input.Items[0].Discount = "0"
				input.Items[0].TaxType = "exempt"
				input.AdditionalExpenses = "0"
			}
			po, inv, a, id, p := orderInvoiceFixture(t, input)
			iid, _ := issueOrderInvoice(t, po, inv, a, id, p)
			invoice, e := inv.Get(ctx, a, iid)
			if e != nil || invoice.Currency != "USD" {
				t.Fatal(invoice, e)
			}
			if zero {
				if invoice.Balance != "0.00" || invoice.PaidTotal != "0.00" || invoice.CommercialStatus == nil || *invoice.CommercialStatus != "paid" {
					t.Fatal(invoice)
				}
				source, e := po.Get(ctx, a, id)
				if e != nil {
					t.Fatal(e)
				}
				v, e := po.CloseInvoice(ctx, a, id, "order-zero-close", purchaseorders.InvoiceClosureInput{Version: source.Version, InvoiceVersion: invoice.Version, Reason: "OC de total cero revisada"}, false)
				if customerResult(t, v, e)["status"] != "closed" {
					t.Fatal(v)
				}
				ledger, e := inv.Payments(ctx, a, iid, 1, 25)
				if e != nil || ledger.Total != 0 {
					t.Fatal(ledger, e)
				}
			}
		})
	}
}
