package integration

import (
	"fmt"
	"sync"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
)

func makeStateOrder(t *testing.T, s *purchaseorders.Service, a core.Actor) string {
	t.Helper()
	v, e := s.Save(ctx, a, "", "po-state-create", purchaseOrderInput("", ""))
	return customerResult(t, v, e)["id"]
}

func stateChange(t *testing.T, s *purchaseorders.Service, a core.Actor, id string, position int, to string) {
	t.Helper()
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	v, e := s.Transition(ctx, a, id, fmt.Sprintf("po-state-%d", o.Version), position, purchaseorders.TransitionInput{ToStatus: to, Version: o.Version})
	if customerResult(t, v, e)["version"] != fmt.Sprint(o.Version+1) {
		t.Fatal(v, e)
	}
}

func TestPurchaseOrderStateFlowDeliveryHistoryAndEditProtection(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	id := makeStateOrder(t, s, a)
	if _, e := s.Transition(ctx, a, id, "po-skip-state", 0, purchaseorders.TransitionInput{ToStatus: "purchasing", Version: 1}); e != core.Conflict {
		t.Fatal("skip allowed", e)
	}
	if _, e := s.Transition(ctx, a, id, "po-position", 2, purchaseorders.TransitionInput{ToStatus: "purchased", Version: 1}); e != core.Missing {
		t.Fatal("unknown item allowed", e)
	}
	stateChange(t, s, a, id, 1, "purchased")
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Status != "received" || o.Items[0].Status != "purchased" {
		t.Fatal("implicit header transition", o, e)
	}
	if _, e := s.Save(ctx, a, id, "po-no-reset-items", editPurchaseOrder(o)); e != core.Conflict {
		t.Fatal("full save reset item progress", e)
	}
	for _, to := range []string{"confirmed", "purchasing", "in_transit", "goods_received", "preparing_delivery"} {
		stateChange(t, s, a, id, 0, to)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-unready-delivery", 0, purchaseorders.TransitionInput{ToStatus: "delivered", Version: o.Version}); e != core.Conflict {
		t.Fatal("delivery with incomplete items", e)
	}
	for _, to := range []string{"shipped", "received", "delivered"} {
		stateChange(t, s, a, id, 1, to)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := purchaseorders.TransitionInput{ToStatus: "delivered", Version: o.Version}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-delivery-reauth", 0, p); e == nil || e.Error() != "reauth_required" {
		t.Fatal("delivery bypassed reauth", e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	v, e := s.Transition(ctx, a, id, "po-delivery-reauth", 0, p)
	if customerResult(t, v, e)["status"] != "delivered" {
		t.Fatal(v, e)
	}
	if _, e := s.Transition(ctx, a, id, "po-delivery-reauth", 0, p); e != nil {
		t.Fatal("historical delivery replay", e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-delivery-reauth", 0, p); e == nil || e.Error() != "reauth_required" {
		t.Fatal("historical delivery replay bypassed live reauth", e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Version != 11 || o.Calculation.Total != "207.00" || o.Calculation.GeneralDiscount != "0.00" || o.Calculation.InvestmentTotal != "70.00" {
		t.Fatal("status changed financials", o, e)
	}
	for _, to := range []string{"invoiced", "closed", "received"} {
		if _, e := s.Transition(ctx, a, id, "po-forbidden-"+to, 0, purchaseorders.TransitionInput{ToStatus: to, Version: o.Version}); e != core.Conflict {
			t.Fatal("reserved/reverse transition allowed", to, e)
		}
	}
	h, e := s.StatusHistory(ctx, a, id, 1, 3)
	if e != nil || h.Total != 10 || len(h.Data) != 3 || h.Data[0].ToStatus != "delivered" || h.Data[0].ItemPosition != nil || h.Data[0].Version != 11 || h.Data[1].ItemPosition == nil || *h.Data[1].ItemPosition != 1 {
		t.Fatal("history", h, e)
	}
	h2, e := s.StatusHistory(ctx, a, id, 2, 3)
	if e != nil || h2.Total != 10 || len(h2.Data) != 3 || h2.Data[0].ID == h.Data[0].ID {
		t.Fatal("history pagination", h2, e)
	}
	var n int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events WHERE entity_id=? AND topic='purchase_order.delivered'`, id).Scan(&n); e != nil || n != 1 {
		t.Fatal("delivery fact count", n, e)
	}
}

func TestPurchaseOrderTransitionConcurrentReplayAndRollback(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	id := makeStateOrder(t, s, a)
	p := purchaseorders.TransitionInput{ToStatus: "confirmed", Version: 1}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_po_state_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-state-rollback", 0, p); e == nil {
		t.Fatal("audit failure ignored")
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Status != "received" || o.Version != 1 {
		t.Fatal("aggregate rollback", o, e)
	}
	h, e := s.StatusHistory(ctx, a, id, 1, 25)
	if e != nil || h.Total != 0 {
		t.Fatal("history rollback", h, e)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER test_po_state_audit`); e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, e := s.Transition(ctx, a, id, "po-state-rollback", 0, p)
			errs <- e
		}()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal("concurrent replay", e)
		}
	}
	h, e = s.StatusHistory(ctx, a, id, 1, 25)
	if e != nil || h.Total != 1 {
		t.Fatal("duplicated history", h, e)
	}
	if _, e := s.Transition(ctx, a, id, "po-state-rollback", 0, purchaseorders.TransitionInput{ToStatus: "purchasing", Version: 2}); e != core.Conflict {
		t.Fatal("payload mismatch", e)
	}
	if _, e := s.Transition(ctx, a, id, "po-state-stale", 0, p); e != core.Conflict {
		t.Fatal("stale transition", e)
	}
	// Independent intents with the same aggregate version produce one winner.
	errs = make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_, e := s.Transition(ctx, a, id, fmt.Sprintf("po-state-competing-%d", i), 1, purchaseorders.TransitionInput{ToStatus: "purchased", Version: 2})
			errs <- e
		}(i)
	}
	wg.Wait()
	close(errs)
	winners := 0
	for e := range errs {
		if e == nil {
			winners++
		} else if e != core.Conflict {
			t.Fatal("competing transition", e)
		}
	}
	if winners != 1 {
		t.Fatal("expected one winner", winners)
	}
	// Item update, header version, history and intention must roll back together.
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_po_state_event BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
		t.Fatal(e)
	}
	p = purchaseorders.TransitionInput{ToStatus: "shipped", Version: 3}
	if _, e := s.Transition(ctx, a, id, "po-item-rollback", 1, p); e == nil {
		t.Fatal("event failure ignored")
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Version != 3 || o.Items[0].Status != "purchased" {
		t.Fatal("item rollback", o, e)
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER test_po_state_event`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-item-rollback", 1, p); e != nil {
		t.Fatal("retry after item rollback", e)
	}
}

func TestPurchaseOrderTransitionScopesRevocationAndNoMachineWrites(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	id := makeStateOrder(t, s, a)
	p := purchaseorders.TransitionInput{ToStatus: "confirmed", Version: 1}
	setScope := func(scope string) {
		t.Helper()
		if _, e := s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope=? WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.transition'`, scope); e != nil {
			t.Fatal(e)
		}
	}
	res, e := s.Core.DB.Exec(`INSERT INTO users(email,name,password_hash,active,created_at,updated_at) SELECT 'po-state-other@example.test','Otro',password_hash,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=?`, a.UserID)
	if e != nil {
		t.Fatal(e)
	}
	n, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE customer_purchase_orders SET created_by=? WHERE id=?`, n, id); e != nil {
		t.Fatal(e)
	}
	setScope("self")
	if _, e := s.Transition(ctx, a, id, "po-state-idor", 0, p); e != core.Missing {
		t.Fatal("self IDOR", e)
	}
	res, e = s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo estados')`)
	if e != nil {
		t.Fatal(e)
	}
	team, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE customer_purchase_orders SET team_id=? WHERE id=?`, team, id); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	setScope("team")
	if _, e := s.Transition(ctx, a, id, "po-state-team", 0, p); e != nil {
		t.Fatal("team transition", e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM team_members WHERE team_id=? AND user_id=?`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-state-team", 0, p); e == nil {
		t.Fatal("revoked membership replay")
	}
	setScope("all")
	if _, e := s.Transition(ctx, a, id, "po-state-team", 0, p); e != nil {
		t.Fatal("all historical replay", e)
	}
	setPurchaseOrderViewScope(t, s.Core, "self")
	if _, e := s.StatusHistory(ctx, a, id, 1, 25); e != core.Missing {
		t.Fatal("history IDOR", e)
	}
	setPurchaseOrderViewScope(t, s.Core, "all")
	if _, e := s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN roles r ON r.id=rp.role_id WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.transition'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, a, id, "po-state-team", 0, p); e != core.Forbidden {
		t.Fatal("revoked grant replay", e)
	}
	machine := a
	machine.Type = "service"
	if _, e := s.Transition(ctx, machine, id, "po-state-machine", 0, p); e != core.Forbidden {
		t.Fatal("machine write", e)
	}
}
