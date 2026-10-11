package integration

import (
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func conversionFixture(t *testing.T) (*quotes.Service, *purchaseorders.Service, core.Actor) {
	t.Helper()
	q, a, _ := quotesFixture(t)
	p := &purchaseorders.Service{Core: q.Core}
	q.PurchaseOrders = p
	grantQuote(t, q.Core, a, "all")
	grantPurchaseOrders(t, q.Core, a, "all")
	return q, p, a
}
func conversionInput() quotes.PurchaseOrderConversionInput {
	return quotes.PurchaseOrderConversionInput{Version: 2, OrderNumber: "OC-CONVERSION-001", Destination: "Destino sintético", OrderDate: "2026-10-11", Comments: "OC recibida", AdditionalExpenses: "3.50"}
}
func acceptedConversionQuote(t *testing.T, s *quotes.Service, a core.Actor, key string, p quotes.Input) string {
	t.Helper()
	id := makeQuote(t, s, a, key, p)
	// SMTP success is covered by email integration/browser flows. This fixture
	// isolates conversion using a sent source then its real acceptance action.
	if _, e := s.Core.DB.Exec(`UPDATE quotes SET status='sent' WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, key+"-accepted", quotes.TransitionInput{Status: "accepted", Version: 1, Reason: "Cliente acepta la oferta sintética"}); e != nil {
		t.Fatal(e)
	}
	return id
}

func TestQuotePurchaseOrderConversionFinancialSnapshotsAndImmutableLinks(t *testing.T) {
	s, p, a := conversionFixture(t)
	input := quoteInput()
	grantCustomer(t, s.Core, a, "all")
	customerID := makeCustomer(t, s.Customers.(*customers.Service), a, "conversion-customer", customerInput("Cliente original"))
	input.Customer.ID = customerID
	input.Currency = "USD"
	input.DiscountType = "percent"
	input.Discount = "7.125"
	input.Exoneration = "Exoneración sintética"
	input.Items = []quotes.Item{
		{Description: "ISV15", Unit: "u", Quantity: "2.125", Price: "100.125125", Cost: "30.005", DiscountType: "amount", Discount: "2.25", Tax: "isv15"},
		{Description: "ISV18", Unit: "u", Quantity: "1.5", Price: "20", Cost: "3.335", DiscountType: "percent", Discount: "10.5", Tax: "isv18"},
		{Description: "Exento", Unit: "u", Quantity: "1", Price: "15", Cost: "2", DiscountType: "amount", Discount: "0", Tax: "exempt"},
		{Description: "Exonerado", Unit: "u", Quantity: "3", Price: "9", Cost: "1", DiscountType: "amount", Discount: "0", Tax: "exonerated"},
	}
	id := acceptedConversionQuote(t, s, a, "conversion-mixed", input)
	if _, e := s.Core.DB.Exec(`UPDATE customers SET name='Nombre cambiado',version=version+1 WHERE id=?`, customerID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission LIKE 'clientes.%'`); e != nil {
		t.Fatal(e)
	}
	before, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	meta := conversionInput()
	v, e := s.ConvertToPurchaseOrder(ctx, a, id, "conversion-first", meta)
	result := customerResult(t, v, e)
	o, e := p.Get(ctx, a, result["purchase_order_id"])
	if e != nil {
		t.Fatal(e)
	}
	if o.SourceQuoteID == nil || *o.SourceQuoteID != id || o.SourceQuoteVersion == nil || *o.SourceQuoteVersion != 2 || o.Version != 1 || o.Status != "received" || o.Currency != "USD" || o.RequiredDeliveryDate != nil || o.Customer.Name != before.Customer.Name || o.Customer.Name != "Cliente original" || o.Customer.ID != customerID || o.CustomerVersion != before.CustomerVersion || o.Calculation.Total != before.Calculation.Total || o.Calculation.GeneralDiscount != before.Calculation.GeneralDiscount || o.Calculation.ProductsInvestment != before.Calculation.Cost || o.Discount != before.Discount || o.Exoneration != input.Exoneration || o.Comments == input.Notes || len(o.Items) != 4 {
		t.Fatal(o, before)
	}
	for i, x := range o.Items {
		y := before.Items[i]
		if x.Status != "pending" || x.SupplierID != "" || x.Tracking != "" || x.Quantity != y.Quantity || x.UnitSalePrice != y.Price || x.UnitCost != y.Cost || x.Discount != y.Discount || x.AllocatedDiscount != y.AllocatedDiscount || x.Base != y.Base {
			t.Fatal(i, x, y)
		}
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Status != "converted" || after.Version != 3 || after.PurchaseOrderID == nil || *after.PurchaseOrderID != o.ID {
		t.Fatal(after, e)
	}
	if _, e = s.Save(ctx, a, id, "converted-edit", editQuote(after)); e != core.Conflict {
		t.Fatal("converted edit", e)
	}
	if _, e = s.Transition(ctx, a, id, "converted-reopen", quotes.TransitionInput{Version: 3, Status: "draft", Reason: "Reabrir"}); e != core.Conflict {
		t.Fatal("converted reopen", e)
	}
	if _, e = s.Trash(ctx, a, id, "converted-trash", 3, false); e != core.Conflict {
		t.Fatal("converted trash", e)
	}
	clone, e := s.Clone(ctx, a, id, "converted-clone", 3)
	cloned, e := s.Get(ctx, a, customerResult(t, clone, e)["id"])
	if e != nil || cloned.PurchaseOrderID != nil || cloned.Status != "draft" {
		t.Fatal(cloned, e)
	}
	edit := editPurchaseOrder(o)
	edit.Comments = "Editada posteriormente"
	if _, e = p.Save(ctx, a, o.ID, "converted-order-edit", edit); e != nil {
		t.Fatal(e)
	}
	if _, e = p.Transition(ctx, a, o.ID, "converted-order-confirm", 0, purchaseorders.TransitionInput{Version: 2, ToStatus: "confirmed"}); e != nil {
		t.Fatal(e)
	}
	replay, e := s.ConvertToPurchaseOrder(ctx, a, id, "conversion-first", meta)
	if customerResult(t, replay, e)["purchase_order_version"] != "1" {
		t.Fatal(replay)
	}
	o, e = p.Get(ctx, a, o.ID)
	if e != nil || o.Version != 3 || o.Status != "confirmed" || *o.SourceQuoteVersion != 2 {
		t.Fatal("replay changed order", o, e)
	}
	if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-another", meta); e != core.Conflict {
		t.Fatal("another intent", e)
	}
	history, e := s.Versions(ctx, a, id, 1, 25)
	if e != nil || history.(map[string]any)["total"] != 3 {
		t.Fatal(history, e)
	}
	var raw string
	if e = s.Core.DB.QueryRow(`SELECT data FROM quote_versions WHERE quote_id=? AND version=3`, id).Scan(&raw); e != nil {
		t.Fatal(e)
	}
	var snap quotes.Quote
	if json.Unmarshal([]byte(raw), &snap) != nil || snap.PurchaseOrderID == nil || *snap.PurchaseOrderID != o.ID {
		t.Fatal(raw)
	}
}

func TestQuotePurchaseOrderConversionConcurrentIntentAndAtomicRollback(t *testing.T) {
	s, p, a := conversionFixture(t)
	id := acceptedConversionQuote(t, s, a, "conversion-concurrent", quoteInput())
	meta := conversionInput()
	var wg sync.WaitGroup
	var successes atomic.Int32
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			v, e := s.ConvertToPurchaseOrder(ctx, a, id, "conversion-concurrent-same", meta)
			if e != nil {
				t.Error(e)
				return
			}
			if v == nil {
				t.Error("empty replay")
				return
			}
			successes.Add(1)
		}()
	}
	wg.Wait()
	if successes.Load() != 12 {
		t.Fatal(successes.Load())
	}
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_orders WHERE source_quote_id=?`, id).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	id2 := acceptedConversionQuote(t, s, a, "conversion-competing", quoteInput())
	successes.Store(0)
	actors := make([]core.Actor, 8)
	for i := range actors {
		actors[i] = customerActor(t, s.Core, "all")
		grantQuote(t, s.Core, actors[i], "all")
		grantPurchaseOrders(t, s.Core, actors[i], "all")
	}
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_, e := s.ConvertToPurchaseOrder(ctx, actors[i], id2, fmt.Sprintf("conversion-competing-%d", i), meta)
			if e == nil {
				successes.Add(1)
			} else if e != core.Conflict {
				t.Error(e)
			}
		}(i)
	}
	wg.Wait()
	if successes.Load() != 1 {
		t.Fatal("competing", successes.Load())
	}
	for _, target := range []string{"audit_events", "domain_events", "quote_versions"} {
		id3 := acceptedConversionQuote(t, s, a, "conversion-rollback-"+target, quoteInput())
		if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_conversion_failure BEFORE INSERT ON ` + target + ` FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
			t.Fatal(e)
		}
		_, e := s.ConvertToPurchaseOrder(ctx, a, id3, "conversion-rollback-key", meta)
		if _, drop := s.Core.DB.Exec(`DROP TRIGGER test_conversion_failure`); drop != nil {
			t.Fatal(drop)
		}
		if e == nil {
			t.Fatal("rollback missing", target)
		}
		q, e := s.Get(ctx, a, id3)
		if e != nil || q.Status != "accepted" || q.Version != 2 || q.PurchaseOrderID != nil {
			t.Fatal(q, e)
		}
		if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_orders WHERE source_quote_id=?`, id3).Scan(&count); e != nil || count != 0 {
			t.Fatal("partial target", target, count, e)
		}
		v, e := s.ConvertToPurchaseOrder(ctx, a, id3, "conversion-rollback-key", meta)
		o, e := p.Get(ctx, a, customerResult(t, v, e)["purchase_order_id"])
		if e != nil || len(o.Items) != 1 {
			t.Fatal(o, e)
		}
	}
}

func TestQuotePurchaseOrderConversionStatesScopesAndReplayRevocation(t *testing.T) {
	s, p, a := conversionFixture(t)
	meta := conversionInput()
	draft := makeQuote(t, s, a, "conversion-draft", quoteInput())
	meta.Version = 1
	for _, state := range []string{"draft", "costing", "review", "approved", "sent", "rejected", "expired"} {
		if _, e := s.Core.DB.Exec(`UPDATE quotes SET status=? WHERE id=?`, state, draft); e != nil {
			t.Fatal(e)
		}
		if _, e := s.ConvertToPurchaseOrder(ctx, a, draft, "conversion-invalid-"+state, meta); e != core.Conflict {
			t.Fatal(state, e)
		}
	}
	id := acceptedConversionQuote(t, s, a, "conversion-scope", quoteInput())
	meta = conversionInput()
	for _, version := range []uint64{0, 1, 3} {
		invalid := meta
		invalid.Version = version
		_, e := s.ConvertToPurchaseOrder(ctx, a, id, "conversion-version", invalid)
		if version == 0 && e != core.Invalid || version != 0 && e != core.Conflict {
			t.Fatal(version, e)
		}
	}
	machine := a
	machine.Type = "service"
	if _, e := s.ConvertToPurchaseOrder(ctx, machine, id, "conversion-machine", meta); e != core.Forbidden {
		t.Fatal(e)
	}
	savedTarget := s.PurchaseOrders
	s.PurchaseOrders = nil
	if _, e := s.ConvertToPurchaseOrder(ctx, a, id, "conversion-disabled", meta); e != core.Missing {
		t.Fatal(e)
	}
	s.PurchaseOrders = savedTarget
	changeScope := func(role, permission, scope string) {
		t.Helper()
		if _, e := s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope=? WHERE r.name=? AND rp.permission=?`, scope, role, permission); e != nil {
			t.Fatal(e)
		}
	}
	res, e := s.Core.DB.Exec(`INSERT INTO users(email,name,password_hash,active,created_at,updated_at) SELECT 'conversion-other@example.test','Otro',password_hash,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=?`, a.UserID)
	if e != nil {
		t.Fatal(e)
	}
	other, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE quotes SET created_by=? WHERE id=?`, other, id); e != nil {
		t.Fatal(e)
	}
	changeScope("quotes-all", "cotizaciones.convert", "self")
	if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-self", meta); e != core.Missing {
		t.Fatal("source IDOR", e)
	}
	res, e = s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo conversión')`)
	if e != nil {
		t.Fatal(e)
	}
	team, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE quotes SET team_id=? WHERE id=?`, team, id); e != nil {
		t.Fatal(e)
	}
	changeScope("quotes-all", "cotizaciones.convert", "all")
	changeScope("purchase-orders-all", "ordenes.create", "self")
	if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-foreign-team", meta); e != core.Forbidden {
		t.Fatal("create outside team", e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	changeScope("quotes-all", "cotizaciones.convert", "team")
	changeScope("purchase-orders-all", "ordenes.create", "team")
	changeScope("purchase-orders-all", "ordenes.view", "team")
	v, e := s.ConvertToPurchaseOrder(ctx, a, id, "conversion-team", meta)
	result := customerResult(t, v, e)
	o, e := p.Get(ctx, a, result["purchase_order_id"])
	if e != nil || o.TeamID != fmt.Sprint(team) || o.CreatedBy != a.UserID {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE team_id=? AND user_id=?`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-team", meta); e == nil {
		t.Fatal("membership revoked replay")
	}
	changeScope("quotes-all", "cotizaciones.convert", "all")
	changeScope("purchase-orders-all", "ordenes.create", "all")
	changeScope("purchase-orders-all", "ordenes.view", "self")
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET created_by=? WHERE id=?`, other, o.ID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-team", meta); e != core.Missing {
		t.Fatal("target replay IDOR", e)
	}
	changeScope("purchase-orders-all", "ordenes.view", "all")
	if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-team", meta); e != nil {
		t.Fatal("authorized replay", e)
	}
	for _, permission := range []string{"cotizaciones.convert", "cotizaciones.view", "ordenes.create", "ordenes.view"} {
		if _, e = s.Core.DB.Exec(`UPDATE role_permissions SET scope='self' WHERE permission=?`, permission); e != nil {
			t.Fatal(e)
		}
		// Remove the grant entirely; source or destination scope must be current.
		if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission=?`, permission); e != nil {
			t.Fatal(e)
		}
		if _, e = s.ConvertToPurchaseOrder(ctx, a, id, "conversion-team", meta); e == nil {
			t.Fatal("permission revoked", permission)
		}
		grantQuote(t, s.Core, a, "all")
		grantPurchaseOrders(t, s.Core, a, "all")
	}
}

func TestQuotePurchaseOrderConversionHTTPStrictAndDisabledContract(t *testing.T) {
	s, _, a := conversionFixture(t)
	id := acceptedConversionQuote(t, s, a, "conversion-http", quoteInput())
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e := r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	path := "/api/v1/quotes/" + id + "/convert-to-purchase-order"
	raw, _ := json.Marshal(conversionInput())
	var good map[string]any
	if json.Unmarshal(raw, &good) != nil {
		t.Fatal("fixture")
	}
	for name := range good {
		body := map[string]any{}
		for k, v := range good {
			body[k] = v
		}
		delete(body, name)
		status, _ := purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, s.Core.Config.Origin, "conversion-http-key")
		if status != 422 {
			t.Fatal("missing", name, status)
		}
	}
	for _, body := range []map[string]any{{"version": 2}, {"version": nil}, {"items": []any{}}, {"currency": "USD"}, {"source_quote_id": "7"}, {"team_id": "7"}} {
		bad := map[string]any{}
		for k, v := range good {
			bad[k] = v
		}
		for k, v := range body {
			bad[k] = v
		}
		status, _ := purchaseOrderHTTP(t, h, "POST", path, bad, cookie, csrf, s.Core.Config.Origin, "conversion-http-key")
		if status != 422 {
			t.Fatal("protected/type", body, status)
		}
	}
	for name := range good {
		if name == "required_delivery_date" {
			continue
		}
		bad := map[string]any{}
		for k, v := range good {
			bad[k] = v
		}
		bad[name] = nil
		status, _ := purchaseOrderHTTP(t, h, "POST", path, bad, cookie, csrf, s.Core.Config.Origin, "conversion-http-key")
		if status != 422 {
			t.Fatal("null", name, status)
		}
	}
	status, _ := purchaseOrderHTTP(t, h, "POST", path, good, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal("missing key", status)
	}
	status, result := purchaseOrderHTTP(t, h, "POST", path, good, cookie, csrf, s.Core.Config.Origin, "conversion-http-key")
	if status != 200 || result["status"] != "converted" || result["purchase_order_id"] == nil {
		t.Fatal(status, result)
	}
	status, _ = purchaseOrderHTTP(t, h, "POST", path, good, cookie, csrf, s.Core.Config.Origin, "conversion-http-key")
	if status != 200 {
		t.Fatal("replay", status)
	}
	s.PurchaseOrders = nil
	r = registry.New()
	if e = r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h = api.New(s.Core, r, nil, logger)
	status, _ = purchaseOrderHTTP(t, h, "POST", path, good, cookie, csrf, s.Core.Config.Origin, "conversion-http-key")
	if status != 404 {
		t.Fatal("disabled route", status)
	}
}
