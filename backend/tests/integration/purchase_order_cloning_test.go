package integration

import (
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
)

func cloneInput() purchaseorders.CloneInput {
	return purchaseorders.CloneInput{Version: 1, OrderNumber: "OC-CLONE-001", OrderDate: "2026-10-11", AdditionalExpenses: "0"}
}

func TestOrderCloneConcurrentReplayOperationalResetDocumentsAndRevocation(t *testing.T) {
	s, a, id, _, doc, _ := orderDocumentFixture(t)
	if _, e := s.LinkDocument(ctx, a, id, "clone-source-document", purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1}); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE customer_purchase_orders SET status='delivered' WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE customer_purchase_order_items SET status='delivered',tracking='TRACK',comments='Operational' WHERE purchase_order_id=?`, id); e != nil {
		t.Fatal(e)
	}
	source, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := cloneInput()
	p.Version = source.Version
	var wg sync.WaitGroup
	results := make(chan string, 12)
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			v, e := s.Clone(ctx, a, id, "clone-concurrent", p)
			if e != nil {
				errs <- e
				return
			}
			raw, _ := json.Marshal(v)
			var r map[string]string
			_ = json.Unmarshal(raw, &r)
			results <- r["id"]
		}()
	}
	wg.Wait()
	close(results)
	close(errs)
	for e := range errs {
		t.Fatal(e)
	}
	var target string
	for v := range results {
		if target != "" && target != v {
			t.Fatal("duplicate", target, v)
		}
		target = v
	}
	o, e := s.Get(ctx, a, target)
	if e != nil {
		t.Fatal(e)
	}
	if o.Status != "received" || o.Version != 1 || o.SourceQuoteID != nil || o.ClonedFromID == nil || *o.ClonedFromID != id || o.ClonedFromVersion == nil || *o.ClonedFromVersion != 2 || o.Calculation.Total != source.Calculation.Total || o.CustomerVersion != source.CustomerVersion {
		t.Fatal(o)
	}
	for i, x := range o.Items {
		if x.ID == source.Items[i].ID || x.Status != "pending" || x.Tracking != "" || x.Comments != "" || x.SupplierVersion != source.Items[i].SupplierVersion {
			t.Fatal(x)
		}
	}
	list, e := s.Documents(ctx, a, target, "", 1, 25, false)
	if e != nil || list.(map[string]any)["total"] != 0 {
		t.Fatal(list, e)
	}
	var histories int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_order_status_history WHERE purchase_order_id=?`, target).Scan(&histories); e != nil || histories != 0 {
		t.Fatal(histories, e)
	}
	latest, e := s.Get(ctx, a, id)
	if e != nil || latest.Version != source.Version || latest.Status != "delivered" {
		t.Fatal(latest, e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET version=version+1 WHERE id IN (?,?)`, id, target); e != nil {
		t.Fatal(e)
	}
	v, e := s.Clone(ctx, a, id, "clone-concurrent", p)
	if customerResult(t, v, e)["id"] != target {
		t.Fatal(v, e)
	}
	changed := p
	changed.OrderNumber = "DIFFERENT"
	if _, e = s.Clone(ctx, a, id, "clone-concurrent", changed); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-stale", p); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET created_by=? WHERE id=?`, customerActor(t, s.Core, "self").UserID, target); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope='self' WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.view'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-concurrent", p); e != core.Missing {
		t.Fatal("destination scope replay", e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN roles r ON r.id=rp.role_id WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.create'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-concurrent", p); e != core.Forbidden {
		t.Fatal("revoked creation", e)
	}
}

func TestOrderCloneRollbackAndSourceScope(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	v, e := s.Save(ctx, a, "", "clone-source", purchaseOrderInput("", ""))
	id := customerResult(t, v, e)["id"]
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER test_po_clone_event BEFORE INSERT ON domain_events FOR EACH ROW BEGIN IF NEW.topic='purchase_order.cloned' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'; END IF; END`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-rollback", cloneInput()); e == nil {
		t.Fatal("event failure ignored")
	}
	var count int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_orders`).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_order_clones`).Scan(&count); e != nil || count != 0 {
		t.Fatal(count, e)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER test_po_clone_event`); e != nil {
		t.Fatal(e)
	}
	v, e = s.Clone(ctx, a, id, "clone-rollback", cloneInput())
	target := customerResult(t, v, e)["id"]
	if target == id {
		t.Fatal(target)
	}
	other := customerActor(t, s.Core, "self")
	grantPurchaseOrders(t, s.Core, other, "self")
	if _, e = s.Clone(ctx, other, id, "clone-foreign", cloneInput()); e != core.Missing {
		t.Fatal(e)
	}
	tech := a
	tech.Type = "service"
	if _, e = s.Clone(ctx, tech, id, "clone-service", cloneInput()); e != core.Forbidden {
		t.Fatal(e)
	}
	p := cloneInput()
	p.OrderNumber = "NEW"
	p.OrderDate = "invalid"
	if _, e = s.Clone(ctx, a, id, "clone-invalid", p); e != core.Invalid {
		t.Fatal(e)
	}
}

func TestOrderCloneHTTPStrictContractAndCSRF(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	v, e := s.Save(ctx, a, "", "clone-http-source", purchaseOrderInput("", ""))
	id := customerResult(t, v, e)["id"]
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e = r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	raw, _ := json.Marshal(cloneInput())
	var valid map[string]any
	_ = json.Unmarshal(raw, &valid)
	path := "/api/v1/purchase-orders/" + id + "/clone"
	for _, mode := range []string{"omitted", "numeric", "unknown", "null", "key"} {
		body := map[string]any{}
		for k, v := range valid {
			body[k] = v
		}
		key := "clone-http-" + mode
		switch mode {
		case "omitted":
			delete(body, "required_delivery_date")
		case "numeric":
			body["version"] = 1
		case "unknown":
			body["items"] = []any{}
		case "null":
			body["destination"] = nil
		case "key":
			key = ""
		}
		status, out := purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, s.Core.Config.Origin, key)
		if status != 422 {
			t.Fatal(mode, status, out)
		}
	}
	status, out := purchaseOrderHTTP(t, h, "POST", path, valid, cookie, "", s.Core.Config.Origin, "clone-http-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, valid, cookie, csrf, s.Core.Config.Origin, "clone-http-valid")
	if status != 200 || out["version"] != "1" || out["id"] == id {
		t.Fatal(status, out)
	}
}

func TestOrderCloneConcurrentWithSourceTransitionUsesConsistentLocks(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	v, e := s.Save(ctx, a, "", "clone-race-source", purchaseOrderInput("", ""))
	id := customerResult(t, v, e)["id"]
	start := make(chan struct{})
	errs := make(chan error, 2)
	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		defer wg.Done()
		<-start
		_, e := s.Clone(ctx, a, id, "clone-transition-race", cloneInput())
		errs <- e
	}()
	go func() {
		defer wg.Done()
		<-start
		_, e := s.Transition(ctx, a, id, "transition-clone-race", 0, purchaseorders.TransitionInput{Version: 1, ToStatus: "confirmed"})
		errs <- e
	}()
	close(start)
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil && e != core.Conflict {
			t.Fatal("lock order failure", e)
		}
	}
	source, e := s.Get(ctx, a, id)
	if e != nil || source.Status != "confirmed" || source.Version != 2 {
		t.Fatal(source, e)
	}
}

func TestOrderCloneHistoricalCatalogSnapshotsAndTeamAccess(t *testing.T) {
	s, cs, ss, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	grantCustomer(t, s.Core, a, "all")
	grantSupplier(t, s.Core, a, "all")
	customer := makeCustomer(t, cs, a, "clone-customer", customerInput("Cliente histórico"))
	supplier := makeSupplier(t, ss, a, "clone-supplier", supplierInput("Proveedor histórico"))
	v, e := s.Save(ctx, a, "", "clone-history-source", purchaseOrderInput(customer, supplier))
	id := customerResult(t, v, e)["id"]
	for _, q := range []string{`UPDATE customers SET name='Cliente actual',version=2 WHERE id=?`, `UPDATE suppliers SET name='Proveedor actual',version=2 WHERE id=?`} {
		target := customer
		if q == `UPDATE suppliers SET name='Proveedor actual',version=2 WHERE id=?` {
			target = supplier
		}
		if _, e = s.Core.DB.Exec(q, target); e != nil {
			t.Fatal(e)
		}
	}
	if _, e = s.Core.DB.Exec(`DELETE ur FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=? AND r.name IN ('customers-all','suppliers-all')`, a.UserID); e != nil {
		t.Fatal(e)
	}
	res, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Clone team')`)
	if e != nil {
		t.Fatal(e)
	}
	n, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	team := core.WrapID(n)
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET team_id=? WHERE id=?`, team, id); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope='self' WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.create'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-team-denied", cloneInput()); e != core.Forbidden {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope='team' WHERE r.name='purchase-orders-all' AND rp.permission IN ('ordenes.create','ordenes.view')`); e != nil {
		t.Fatal(e)
	}
	v, e = s.Clone(ctx, a, id, "clone-team-history", cloneInput())
	target := customerResult(t, v, e)["id"]
	o, e := s.Get(ctx, a, target)
	if e != nil || o.Customer.Name != "Cliente histórico" || o.CustomerVersion != 1 || o.Items[0].SupplierName != "Proveedor histórico" || o.Items[0].SupplierVersion != 1 || o.TeamID != team {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE team_id=? AND user_id=?`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-team-history", cloneInput()); e == nil {
		t.Fatal("lost membership replay")
	}
}
