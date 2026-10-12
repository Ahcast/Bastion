package integration

import (
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
)

func invoiceFixture(t *testing.T) (*invoices.Service, core.Actor) {
	cs, a, _ := customersFixture(t)
	return &invoices.Service{Core: cs.Core, Customers: cs}, a
}
func grantInvoice(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	name := "invoices-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var id string
	if e := s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, name).Scan(&id); e != nil {
		t.Fatal(e)
	}
	for _, p := range invoices.Permissions {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, id, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, id); e != nil {
		t.Fatal(e)
	}
}
func invoiceInput() invoices.Input {
	return invoices.Input{Customer: invoices.CustomerInput{Name: "Cliente factura sintético"}, InvoiceDate: "2026-10-11", PaymentTerm: "contado", Currency: "HNL", PublicNotes: "Notas públicas", DiscountType: "amount", Discount: "5", AdditionalExpenses: "9.99", Items: []invoices.ItemInput{{Item: finance.Item{Description: "Equipo sintético", Unit: "u", Quantity: "2", Price: "100", Cost: "40", DiscountType: "amount", Discount: "10", Tax: "isv15"}}}}
}
func editInvoice(o invoices.Invoice) invoices.Input {
	p := invoices.Input{Customer: o.Customer, InvoiceDate: o.InvoiceDate, PaymentTerm: o.PaymentTerm, CreditDays: o.CreditDays, Currency: o.Currency, PublicNotes: o.PublicNotes, DiscountType: o.DiscountType, Discount: o.Discount, Exoneration: o.Exoneration, AdditionalExpenses: o.Calculation.AdditionalExpenses, TeamID: o.TeamID, Version: o.Version}
	for _, x := range o.Items {
		p.Items = append(p.Items, invoices.ItemInput{ID: x.ID, Item: x.Item})
	}
	return p
}
func TestInvoiceDraftRoundTripConditionsStableItemsAndSearch(t *testing.T) {
	s, a := invoiceFixture(t)
	p := invoiceInput()
	if _, e := s.Save(ctx, a, "", "invoice-no-permission", p); e != core.Forbidden {
		t.Fatal(e)
	}
	grantInvoice(t, s.Core, a, "all")
	v, e := s.Save(ctx, a, "", "invoice-create", p)
	id := customerResult(t, v, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Version != 1 || o.Status != "draft" || o.Number != nil || o.SAR != nil || o.PaidTotal != "0.00" || o.Balance != "201.25" || o.Calculation.Profit != "85.01" || o.Items[0].ID == "" {
		t.Fatal(o, e)
	}
	p = editInvoice(o)
	days := uint16(30)
	p.PaymentTerm = "credito"
	p.CreditDays = &days
	p.Currency = "USD"
	p.Items = append(p.Items, invoices.ItemInput{Item: finance.Item{Description: "Complemento", Unit: "u", Quantity: "1", Price: "0", Cost: "0", DiscountType: "amount", Discount: "0", Tax: "exempt"}})
	v, e = s.Save(ctx, a, id, "invoice-edit-credit", p)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	edited, e := s.Get(ctx, a, id)
	if e != nil || edited.Currency != "USD" || edited.CreditDays == nil || *edited.CreditDays != 30 || edited.Items[0].ID != o.Items[0].ID || len(edited.Items) != 2 || edited.Items[1].ID == edited.Items[0].ID {
		t.Fatal(edited, e)
	}
	p = editInvoice(edited)
	p.Items[0], p.Items[1] = p.Items[1], p.Items[0]
	if _, e = s.Save(ctx, a, id, "invoice-reorder", p); e != nil {
		t.Fatal(e)
	}
	reordered, e := s.Get(ctx, a, id)
	if e != nil || reordered.Items[0].ID != edited.Items[1].ID || reordered.Items[1].ID != edited.Items[0].ID || reordered.Calculation != edited.Calculation {
		t.Fatal(reordered, e)
	}
	page, e := s.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25, Search: "Complemento", Status: "draft"})
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != id {
		t.Fatal(page, e)
	}
	p = editInvoice(reordered)
	p.Items[0].ID = "999999"
	if _, e = s.Save(ctx, a, id, "invoice-foreign-item", p); e != core.Invalid {
		t.Fatal(e)
	}
}
func TestInvoiceDraftCustomerHistoricalSnapshotAndCurrentAuthority(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	grantCustomer(t, s.Core, a, "all")
	cs := s.Customers
	customerID := makeCustomer(t, cs.(*customers.Service), a, "invoice-customer", customerInput("Cliente original factura"))
	p := invoiceInput()
	p.Customer.ID = customerID
	p.Customer.Name = "Texto ignorado"
	v, e := s.Save(ctx, a, "", "invoice-customer-source", p)
	id := customerResult(t, v, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Customer.Name != "Cliente original factura" || o.CustomerVersion != 1 {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE customers SET name='Nombre cambiado',version=version+1,is_active=FALSE WHERE id=?`, customerID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='clientes.view'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	p = editInvoice(o)
	p.PublicNotes = "Edición con snapshot histórico"
	if _, e = s.Save(ctx, a, id, "invoice-historical-edit", p); e != nil {
		t.Fatal(e)
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Customer != o.Customer || after.CustomerVersion != o.CustomerVersion {
		t.Fatal(after, e)
	}
	other := customerActor(t, s.Core, "self")
	grantInvoice(t, s.Core, other, "self")
	if _, e = s.Get(ctx, other, id); e != core.Missing {
		t.Fatal(e)
	}
	page, e := s.List(ctx, other, invoices.Filter{Page: 1, PerPage: 25})
	if e != nil || page.Total != 0 {
		t.Fatal(page, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='facturas.update'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "invoice-historical-edit", p); e == nil {
		t.Fatal("revoked replay")
	}
}
func TestInvoiceDraftConcurrentIntentVersionAndRollback(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	p := invoiceInput()
	type reply struct {
		v any
		e error
	}
	replies := make(chan reply, 12)
	var wg sync.WaitGroup
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); v, e := s.Save(ctx, a, "", "invoice-same-intent", p); replies <- reply{v, e} }()
	}
	wg.Wait()
	close(replies)
	id := ""
	for r := range replies {
		got := customerResult(t, r.v, r.e)["id"]
		if id != "" && got != id {
			t.Fatal(got, id)
		}
		id = got
	}
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM invoices`).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	p.PublicNotes = "Otra intención"
	if _, e := s.Save(ctx, a, "", "invoice-same-intent", p); e != core.Conflict {
		t.Fatal(e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p = editInvoice(o)
	results := make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(n int) {
			defer wg.Done()
			input := p
			input.PublicNotes = fmt.Sprint(n)
			_, e := s.Save(ctx, a, id, fmt.Sprintf("invoice-version-%d", n), input)
			results <- e
		}(i)
	}
	wg.Wait()
	close(results)
	success := 0
	for e := range results {
		if e == nil {
			success++
		} else if e != core.Conflict {
			t.Fatal(e)
		}
	}
	if success != 1 {
		t.Fatal(success)
	}
	before, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p = editInvoice(before)
	p.PublicNotes = "Rollback"
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER invoice_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic invoice failure'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "invoice-rollback", p); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER invoice_event_failure`); e != nil {
		t.Fatal(e)
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Version != before.Version || after.PublicNotes != before.PublicNotes || after.Items[0].ID != before.Items[0].ID {
		t.Fatal(after, e)
	}
	if _, e = s.Save(ctx, a, id, "invoice-rollback", p); e != nil {
		t.Fatal(e)
	}
}
func TestInvoiceDraftHTTPStrictFieldsCSRFAndProtectedState(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
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
	raw, _ := json.Marshal(invoiceInput())
	var body map[string]any
	if e = json.Unmarshal(raw, &body); e != nil {
		t.Fatal(e)
	}
	for name := range body {
		clone := map[string]any{}
		for k, v := range body {
			clone[k] = v
		}
		delete(clone, name)
		status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices", clone, cookie, csrf, s.Core.Config.Origin, "invoice-http-missing")
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["invoice_number"] = "UNAUTHORIZED"
	status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices", body, cookie, csrf, s.Core.Config.Origin, "invoice-http-protected")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "invoice_number")
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices", body, cookie, "", s.Core.Config.Origin, "invoice-http-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices", body, cookie, csrf, s.Core.Config.Origin, "invoice-http-valid")
	if status != 200 {
		t.Fatal(status, out)
	}
	id := out["id"].(string)
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := editInvoice(o)
	// Exercise the write guard without reading an intentionally incomplete emission.
	if _, e = s.Core.DB.Exec(`UPDATE invoices SET status='issued' WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "invoice-protected-status", p); e != core.Conflict {
		t.Fatal(e)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", "/api/v1/invoices?status=invalid", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal(status, out)
	}
}

func TestInvoiceDraftTeamScopesTransferReauthAndRevocation(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	res, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Facturas equipo uno')`)
	if e != nil {
		t.Fatal(e)
	}
	n, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	team := core.WrapID(n)
	res, e = s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Facturas equipo dos')`)
	if e != nil {
		t.Fatal(e)
	}
	n, e = res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	team2 := core.WrapID(n)
	member := customerActor(t, s.Core, "self")
	grantInvoice(t, s.Core, member, "team")
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, member.UserID); e != nil {
		t.Fatal(e)
	}
	p := invoiceInput()
	p.TeamID = team
	v, e := s.Save(ctx, member, "", "invoice-team-create", p)
	id := customerResult(t, v, e)["id"]
	o, e := s.Get(ctx, member, id)
	if e != nil || o.TeamID != team {
		t.Fatal(o, e)
	}
	p = editInvoice(o)
	p.TeamID = team2
	if _, e = s.Save(ctx, member, id, "invoice-team-forbidden-transfer", p); e == nil {
		t.Fatal("team-only transfer")
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "invoice-team-transfer", p); e == nil {
		t.Fatal("reauth required")
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "invoice-team-transfer", p); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Get(ctx, member, id); e != core.Missing {
		t.Fatal(e)
	}
	page, e := s.List(ctx, member, invoices.Filter{Page: 1, PerPage: 25})
	if e != nil || page.Total != 0 {
		t.Fatal(page, e)
	}
	if _, e = s.Save(ctx, member, "", "invoice-team-create", invoiceInput()); e == nil {
		t.Fatal("changed creation intent")
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE user_id=?`, member.UserID); e != nil {
		t.Fatal(e)
	}
	original := invoiceInput()
	original.TeamID = team
	if _, e = s.Save(ctx, member, "", "invoice-team-create", original); e == nil {
		t.Fatal("membership revoked replay")
	}
}
