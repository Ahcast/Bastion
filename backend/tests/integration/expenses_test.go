package integration

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/expenses"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
)

func expenseFixture(t *testing.T) (*expenses.Service, core.Actor) {
	ss, a, _ := suppliersFixture(t)
	return &expenses.Service{Core: ss.Core, Suppliers: ss}, a
}
func grantExpense(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	name := "expenses-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var role string
	if e := s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, name).Scan(&role); e != nil {
		t.Fatal(e)
	}
	for _, p := range expenses.Permissions {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, role, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, role); e != nil {
		t.Fatal(e)
	}
}
func expenseInput() expenses.Input {
	return expenses.Input{InvoiceNumber: "DOC-GASTO-1", Date: "2026-10-12", Supplier: expenses.Supplier{Name: "Proveedor libre sintético"}, Category: "freight", CalculationInput: expenses.CalculationInput{Currency: "HNL", Taxable: "100", Exempt: "20"}, Notes: "Gasto sintético"}
}
func makeExpense(t *testing.T, s *expenses.Service, a core.Actor, key string, p expenses.Input) string {
	t.Helper()
	v, e := s.Save(ctx, a, "", key, p)
	return customerResult(t, v, e)["id"]
}
func TestExpenseRegistryCalculationFiltersAndCurrencyTotals(t *testing.T) {
	s, a := expenseFixture(t)
	if _, e := s.Save(ctx, a, "", "expense-no-grant", expenseInput()); e != core.Forbidden {
		t.Fatal(e)
	}
	grantExpense(t, s.Core, a, "all")
	id := makeExpense(t, s, a, "expense-register", expenseInput())
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Calculation.Total != "135.00" || o.Calculation.Tax != "15.00" || o.Version != 1 || o.SupplierVersion != 0 || o.DeletedAt != nil {
		t.Fatal(o, e)
	}
	usd := expenseInput()
	usd.Currency = "USD"
	usd.Taxable = "8000000000000000.01"
	usd.Exempt = "0"
	usd.InvoiceNumber = "USD-DOC"
	makeExpense(t, s, a, "expense-usd", usd)
	totals, e := s.Totals(ctx, a, expenses.Filter{Page: 1, PerPage: 25})
	if e != nil || len(totals) != 2 || totals[0].Currency != "HNL" || totals[0].Total != "135.00" || totals[1].Currency != "USD" || totals[1].Total != "9200000000000000.01" {
		t.Fatal(totals, e)
	}
	f := expenses.Filter{Search: "DOC-GASTO", Currency: "HNL", Category: "freight", From: "2026-10-12", To: "2026-10-12", Page: 1, PerPage: 1}
	list, e := s.List(ctx, a, f)
	if e != nil || list.Total != 1 || list.Data[0].ID != id {
		t.Fatal(list, e)
	}
	for _, f := range []expenses.Filter{{Page: 0, PerPage: 25}, {Page: 1, PerPage: 101}, {Page: 1, PerPage: 25, From: "2026-02-30"}, {Page: 1, PerPage: 25, Currency: "EUR"}, {Page: 1, PerPage: 25, From: "2026-10-13", To: "2026-10-12"}} {
		if _, e = s.List(ctx, a, f); e != core.Invalid {
			t.Fatal(f, e)
		}
	}
}
func TestExpenseSupplierSnapshotAndScopedDuplicates(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantSupplier(t, s.Core, a, "all")
	// Resolve via owner service rather than reading/writing supplier state from Expenses.
	supplierService, ok := s.Suppliers.(interface {
		Save(context.Context, core.Actor, string, string, suppliers.Input) (any, error)
		Get(context.Context, core.Actor, string) (suppliers.Supplier, error)
	})
	if !ok {
		t.Fatal("supplier port")
	}
	pSupplier := supplierInput("Proveedor histórico")
	v, e := supplierService.Save(ctx, a, "", "expense-supplier", pSupplier)
	sid := customerResult(t, v, e)["id"]
	p := expenseInput()
	p.Supplier = expenses.Supplier{ID: sid, Name: "Nombre cliente ignorado"}
	id := makeExpense(t, s, a, "expense-supplier-record", p)
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Supplier.Name != "Proveedor histórico" || o.SupplierVersion != 1 {
		t.Fatal(o, e)
	}
	current, e := supplierService.Get(ctx, a, sid)
	if e != nil {
		t.Fatal(e)
	}
	change := current.Input
	change.Name = "Nombre nuevo"
	change.Active = false
	if _, e = supplierService.Save(ctx, a, sid, "expense-supplier-change", change); e != nil {
		t.Fatal(e)
	}
	p = o.Input
	p.Notes = "Cambio sin refrescar proveedor"
	p.Supplier.Name = "Intento sobrescribir histórico"
	if _, e = s.Save(ctx, a, id, "expense-preserve-supplier", p); e != nil {
		t.Fatal(e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Supplier.Name != "Proveedor histórico" || o.SupplierVersion != 1 {
		t.Fatal(o, e)
	}
	free := expenseInput()
	one := makeExpense(t, s, a, "expense-duplicate-one", free)
	two := makeExpense(t, s, a, "expense-duplicate-two", free)
	dups, e := s.Duplicates(ctx, a, one, 1, 25)
	if e != nil || dups.Total != 1 || dups.Data[0].ID != two {
		t.Fatal(dups, e)
	}
	other := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, other, "self")
	their := makeExpense(t, s, other, "expense-other-duplicate", free)
	d, e := s.Duplicates(ctx, other, their, 1, 25)
	if e != nil || d.Total != 0 {
		t.Fatal(d, e)
	}
	if _, e = s.Get(ctx, other, one); e != core.Missing {
		t.Fatal(e)
	}
}
func TestExpenseConcurrentIntentVersionRollbackAndRevocation(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	type response struct {
		v any
		e error
	}
	replies := make(chan response, 12)
	var wg sync.WaitGroup
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			v, e := s.Save(ctx, a, "", "expense-concurrent-intent", expenseInput())
			replies <- response{v, e}
		}()
	}
	wg.Wait()
	close(replies)
	id := ""
	for r := range replies {
		x := customerResult(t, r.v, r.e)["id"]
		if id != "" && x != id {
			t.Fatal(x, id)
		}
		id = x
	}
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	errs := make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			p := o.Input
			p.Notes = fmt.Sprint(i)
			_, e := s.Save(ctx, a, id, fmt.Sprintf("expense-edit-%d", i), p)
			errs <- e
		}(i)
	}
	wg.Wait()
	close(errs)
	ok := 0
	for e := range errs {
		if e == nil {
			ok++
		} else if e != core.Conflict {
			t.Fatal(e)
		}
	}
	if ok != 1 {
		t.Fatal(ok)
	}
	before, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := before.Input
	p.Taxable = "999"
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER expense_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic expense failure'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "expense-rollback", p); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER expense_event_failure`); e != nil {
		t.Fatal(e)
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Version != before.Version || after.Calculation != before.Calculation {
		t.Fatal(after, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='gastos.create'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, "", "expense-concurrent-intent", expenseInput()); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestExpenseSoftDeleteRestoreReauthAndProtectedEvidence(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	id := makeExpense(t, s, a, "expense-trash-source", expenseInput())
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	state := expenses.StateInput{Version: o.Version, Reason: "Documento duplicado revisado"}
	if _, e = s.State(ctx, a, id, "expense-delete-reauth", state, false); e == nil || e.(*core.Error).Code != "reauth_required" {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	v, e := s.State(ctx, a, id, "expense-delete-once", state, false)
	r := customerResult(t, v, e)
	v, e = s.State(ctx, a, id, "expense-delete-once", state, false)
	if customerResult(t, v, e)["version"] != r["version"] {
		t.Fatal("replay")
	}
	trash, e := s.Get(ctx, a, id)
	if e != nil || trash.DeletedAt == nil || trash.Calculation != o.Calculation {
		t.Fatal(trash, e)
	}
	if _, e = s.Save(ctx, a, id, "expense-edit-deleted", trash.Input); e != core.Conflict {
		t.Fatal(e)
	}
	active, e := s.List(ctx, a, expenses.Filter{Page: 1, PerPage: 25})
	if e != nil || active.Total != 0 {
		t.Fatal(active, e)
	}
	deleted, e := s.List(ctx, a, expenses.Filter{Deleted: true, Page: 1, PerPage: 25})
	if e != nil || deleted.Total != 1 {
		t.Fatal(deleted, e)
	}
	if _, e = s.State(ctx, a, id, "expense-restore", expenses.StateInput{Version: trash.Version, Reason: "Restauración revisada"}, true); e != nil {
		t.Fatal(e)
	}
	restored, e := s.Get(ctx, a, id)
	if e != nil || restored.Version != 3 || restored.DeletedAt != nil || restored.Calculation != o.Calculation || restored.Notes != o.Notes {
		t.Fatal(restored, e)
	}
	var n int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM audit_events WHERE entity_type='expenses' AND entity_id=? AND action IN ('expense.deleted','expense.restored')`, id).Scan(&n); e != nil || n != 2 {
		t.Fatal(n, e)
	}
}
func TestExpenseHTTPStrictCSRFAndFilters(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
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
	raw, _ := json.Marshal(expenseInput())
	var body map[string]any
	_ = json.Unmarshal(raw, &body)
	for name := range body {
		copy := map[string]any{}
		for k, v := range body {
			copy[k] = v
		}
		delete(copy, name)
		status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/expenses", copy, cookie, csrf, s.Core.Config.Origin, "expense-http-missing")
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["tax_amount"] = "0"
	status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/expenses", body, cookie, csrf, s.Core.Config.Origin, "expense-http-injected")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "tax_amount")
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/expenses", body, cookie, "", s.Core.Config.Origin, "expense-http-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/expenses", body, cookie, csrf, s.Core.Config.Origin, "expense-http-valid")
	if status != 200 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", "/api/v1/expenses?currency=EUR", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", "/api/v1/expenses?unknown=1", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal(status, out)
	}
}

func TestExpenseTeamScopeTransferAndRevokedMembershipReplay(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	res, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo gastos uno')`)
	if e != nil {
		t.Fatal(e)
	}
	n, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	team := core.WrapID(n)
	res, e = s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo gastos dos')`)
	if e != nil {
		t.Fatal(e)
	}
	n, e = res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	otherTeam := core.WrapID(n)
	member := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, member, "team")
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, member.UserID); e != nil {
		t.Fatal(e)
	}
	p := expenseInput()
	p.TeamID = team
	id := makeExpense(t, s, member, "expense-team-create", p)
	o, e := s.Get(ctx, member, id)
	if e != nil {
		t.Fatal(e)
	}
	transfer := o.Input
	transfer.TeamID = otherTeam
	if _, e = s.Save(ctx, member, id, "expense-team-denied", transfer); e == nil {
		t.Fatal("team transfer should be denied")
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "expense-team-transfer", transfer); e == nil {
		t.Fatal("reauth required")
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "expense-team-transfer", transfer); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Get(ctx, member, id); e != core.Missing {
		t.Fatal(e)
	}
	list, e := s.List(ctx, member, expenses.Filter{Page: 1, PerPage: 25})
	if e != nil || list.Total != 0 {
		t.Fatal(list, e)
	}
	totals, e := s.Totals(ctx, member, expenses.Filter{Page: 1, PerPage: 25})
	if e != nil || len(totals) != 0 {
		t.Fatal(totals, e)
	}
	if _, e = s.Save(ctx, member, "", "expense-team-create", p); e == nil {
		t.Fatal("destination permission replay must be reauthorized")
	}
}
