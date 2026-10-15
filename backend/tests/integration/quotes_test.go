package integration

import (
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/composition"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"sync"
	"sync/atomic"
	"testing"
)

func quotesFixture(t *testing.T) (*quotes.Service, core.Actor, *registry.Registry) {
	c, a, r := customersFixture(t)
	return &quotes.Service{Core: c.Core, Customers: c, Processes: &processes.Service{Core: c.Core}}, a, r
}
func grantQuote(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	name := "quotes-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var id string
	if e := s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, name).Scan(&id); e != nil {
		t.Fatal(e)
	}
	for _, p := range quotes.Permissions {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, id, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, id); e != nil {
		t.Fatal(e)
	}
}
func quoteInput() quotes.Input {
	return quotes.Input{Customer: quotes.CustomerInput{Name: "Cliente sintético"}, Date: "2026-10-10", Currency: "HNL", ValidityDays: 30, Terms: "Términos públicos", Notes: "Nota privada sintética", CalculationInput: quotes.CalculationInput{Items: []quotes.Item{{Description: "Equipo", Unit: "unidad", Quantity: "2", Price: "100", Cost: "50", DiscountType: "amount", Discount: "10", Tax: "isv15"}}, DiscountType: "amount", Discount: "5"}}
}
func makeQuote(t *testing.T, s *quotes.Service, a core.Actor, key string, p quotes.Input) string {
	t.Helper()
	v, e := s.Save(ctx, a, "", key, p)
	return customerResult(t, v, e)["id"]
}
func editQuote(q quotes.Quote) quotes.Input {
	items := []quotes.Item{}
	for _, x := range q.Items {
		items = append(items, x.Item)
	}
	return quotes.Input{Customer: q.Customer, Reference: q.Reference, Date: q.Date, Currency: q.Currency, ValidityDays: q.ValidityDays, Terms: q.Terms, Notes: q.Notes, AuthorContact: q.AuthorContact, CalculationInput: quotes.CalculationInput{Items: items, DiscountType: q.DiscountType, Discount: q.Discount, Exoneration: q.Exoneration}, Version: q.Version}
}
func TestQuotesAggregateCalculatorVersionsCloneAndTrash(t *testing.T) {
	s, a, _ := quotesFixture(t)
	if _, e := s.Save(ctx, a, "", "quote-ungranted", quoteInput()); e != core.Forbidden {
		t.Fatal(e)
	}
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "quote-create", quoteInput())
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Number != "COT-000001" || q.Calculation.Total != "201.25" || q.Items[0].Base != "175.00" || q.Version != 1 {
		t.Fatal(q, e)
	}
	p := editQuote(q)
	p.Terms = "Otros términos"
	v, e := s.Save(ctx, a, id, "quote-edit", p)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	if _, e = s.Save(ctx, a, id, "quote-stale", p); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "quote-edit", p); e != nil {
		t.Fatal("replay", e)
	}
	v, e = s.Clone(ctx, a, id, "quote-clone", 2)
	clone := customerResult(t, v, e)["id"]
	cq, e := s.Get(ctx, a, clone)
	if e != nil || cq.Number != "COT-000002" || cq.Status != "draft" || cq.Calculation.Total != q.Calculation.Total {
		t.Fatal(cq, e)
	}
	if _, e = s.Trash(ctx, a, id, "quote-delete", 2, false); e != nil {
		t.Fatal(e)
	}
	list, e := s.List(ctx, a, quotes.Filter{Page: 1, PerPage: 5})
	if e != nil || list.Total != 1 {
		t.Fatal(list, e)
	}
	list, e = s.List(ctx, a, quotes.Filter{Page: 1, PerPage: 5, Deleted: true})
	if e != nil || list.Total != 1 {
		t.Fatal(list, e)
	}
	if _, e = s.Trash(ctx, a, id, "quote-restore", 3, true); e != nil {
		t.Fatal(e)
	}
	history, e := s.Versions(ctx, a, id, 1, 25)
	if e != nil || history.(map[string]any)["total"] != 4 {
		t.Fatal(history, e)
	}
	list, e = s.List(ctx, a, quotes.Filter{Page: 1, PerPage: 5, Search: "%"})
	if e != nil || list.Total != 0 {
		t.Fatal(list, e)
	}
	list, e = s.List(ctx, a, quotes.Filter{Page: 1, PerPage: 1, Search: "Equipo"})
	if e != nil || list.Total != 2 || len(list.Data) != 1 {
		t.Fatal(list, e)
	}
}
func TestQuotesConcurrencyNumberingIntentionAndRollback(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	actors := []core.Actor{}
	for i := 0; i < 8; i++ {
		x := customerActor(t, s.Core, "self")
		grantQuote(t, s.Core, x, "all")
		actors = append(actors, x)
	}
	var wg sync.WaitGroup
	for i, a := range actors {
		wg.Add(1)
		go func(i int, a core.Actor) {
			defer wg.Done()
			if _, e := s.Save(ctx, a, "", fmt.Sprint("quote-unique-", i), quoteInput()); e != nil {
				t.Error(e)
			}
		}(i, a)
	}
	wg.Wait()
	var n int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(DISTINCT number) FROM quotes`).Scan(&n); e != nil || n != 8 {
		t.Fatal(n, e)
	}
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, e := s.Save(ctx, a, "", "quote-same-intent", quoteInput()); e != nil {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quotes`).Scan(&n); e != nil || n != 9 {
		t.Fatal(n, e)
	}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_quote_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Save(ctx, a, "", "quote-rollback", quoteInput()); e == nil {
		t.Fatal("audit failure swallowed")
	}
	for _, table := range []string{"quotes", "quote_items", "quote_versions", "idempotency_records", "domain_events"} {
		if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ` + table).Scan(&n); e != nil || n != 9 {
			t.Fatal(table, n, e)
		}
	}
	var next int
	if e := s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='quote'`).Scan(&next); e != nil || next != 10 {
		t.Fatal(next, e)
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER test_quote_audit`); e != nil {
		t.Fatal(e)
	}
	id := makeQuote(t, s, a, "quote-rollback", quoteInput())
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Number != "COT-000010" {
		t.Fatal(q, e)
	}
	p := editQuote(q)
	var success, conflict atomic.Int32
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_, e := s.Save(ctx, a, id, fmt.Sprint("quote-race-edit-", i), p)
			if e == nil {
				success.Add(1)
			} else if e == core.Conflict {
				conflict.Add(1)
			} else {
				t.Error(e)
			}
		}(i)
	}
	wg.Wait()
	if success.Load() != 1 || conflict.Load() != 7 {
		t.Fatal(success.Load(), conflict.Load())
	}
}
func TestQuotesCustomerSnapshotsAndProcessConversion(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	grantCustomer(t, s.Core, a, "all")
	cs := &customers.Service{Core: s.Core}
	customerID := makeCustomer(t, cs, a, "quote-customer", customerInput("Entidad sintética"))
	// Source rows are synthetic fixtures; conversion itself uses Processes service.
	_, e := s.Core.DB.Exec(`INSERT INTO processes(source,external_id,source_url,entity,reference,description,deadline,normalized_data,field_clocks,content_hash,created_by,first_seen_at,last_seen_at,version) VALUES('test','p1','','Entidad sintética','EXP-1','No copiar especificaciones','','{}','{}',REPEAT('a',64),?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),1)`, a.UserID)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.CreateFromProcess(ctx, a, "1", "process-quote-denied", quotes.FromProcessInput{Date: "2026-10-10"}); e != core.Missing {
		t.Fatal(e)
	}
	grantProcess(t, s.Core, a)
	v, e := s.CreateFromProcess(ctx, a, "1", "process-quote-1", quotes.FromProcessInput{Date: "2026-10-10"})
	id := customerResult(t, v, e)["id"]
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Source != "1" || q.Reference != "EXP-1" || q.Customer.ID != customerID || len(q.Items) != 0 || q.Terms != "" || q.Notes != "" {
		t.Fatal(q, e)
	}
	c, e := cs.Get(ctx, a, customerID)
	if e != nil {
		t.Fatal(e)
	}
	c.Name = "Entidad editada"
	if _, e = cs.Save(ctx, a, customerID, "quote-customer-edit", c.Input); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE processes SET entity='Entidad desconocida',reference='EXP-2' WHERE id=1`); e != nil {
		t.Fatal(e)
	}
	v, e = s.CreateFromProcess(ctx, a, "1", "process-quote-1", quotes.FromProcessInput{Date: "2026-10-10"})
	if customerResult(t, v, e)["id"] != id {
		t.Fatal(v)
	}
	v, e = s.CreateFromProcess(ctx, a, "1", "process-quote-2", quotes.FromProcessInput{Date: "2026-10-10"})
	q2, e := s.Get(ctx, a, customerResult(t, v, e)["id"])
	if e != nil || q2.Customer.ID != "" || q2.Customer.Name != "Entidad desconocida" || q2.Reference != "EXP-2" {
		t.Fatal(q2, e)
	}
	p := editQuote(q)
	p.Reference = "Referencia visible editada"
	if _, e = s.Save(ctx, a, id, "quote-edit-reference", p); e != nil {
		t.Fatal(e)
	}
	q, e = s.Get(ctx, a, id)
	if e != nil || q.Customer.Name != "Entidad sintética" || q.Source != "1" {
		t.Fatal(q, e)
	}
	related, e := s.Related(ctx, a, "1", quotes.Filter{Page: 1, PerPage: 5})
	if e != nil || related.Total != 2 {
		t.Fatal(related, e)
	}
	// No customer view grant: manual snapshot, never a catalogue-existence leak.
	other := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, other, "self")
	grantProcess(t, s.Core, other)
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE role_id IN(SELECT role_id FROM user_roles WHERE user_id=?) AND permission='clientes.view'`, other.UserID); e != nil {
		t.Fatal(e)
	}
	v, e = s.CreateFromProcess(ctx, other, "1", "process-no-customer-view", quotes.FromProcessInput{Date: "2026-10-10"})
	q3, e := s.Get(ctx, other, customerResult(t, v, e)["id"])
	if e != nil || q3.Customer.ID != "" {
		t.Fatal(q3, e)
	}
}
func TestQuotesScopesRevocationReauthAndProtectedInputs(t *testing.T) {
	s, a, r := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	owner := customerActor(t, s.Core, "self")
	other := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, owner, "self")
	grantQuote(t, s.Core, other, "self")
	id := makeQuote(t, s, owner, "quote-own", quoteInput())
	if _, e := s.Get(ctx, other, id); e != core.Missing {
		t.Fatal(e)
	}
	list, e := s.List(ctx, other, quotes.Filter{Page: 1, PerPage: 5})
	if e != nil || list.Total != 0 {
		t.Fatal(list, e)
	}
	teamActor := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, teamActor, "team")
	if _, e = s.Core.DB.Exec(`INSERT INTO teams(id,name) VALUES(1,'Prueba')`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(1,?)`, teamActor.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE quotes SET team_id=1 WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	list, e = s.List(ctx, teamActor, quotes.Filter{Page: 1, PerPage: 5})
	if e != nil || list.Total != 1 {
		t.Fatal(list, e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 10 MINUTE) WHERE user_id=?`, owner.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, owner, id, "quote-no-reauth", 1, false); e == nil {
		t.Fatal("missing reauth")
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM user_roles WHERE user_id=?`, owner.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, owner, "", "quote-own", quoteInput()); e != core.Forbidden {
		t.Fatal("cached replay bypass", e)
	}
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	status, _, _ := execute(t, h, "POST", "/api/v1/quotes", map[string]any{"created_by": "2", "version": "0"}, cookie, csrf, s.Core.Config.Origin)
	if status != 422 {
		t.Fatal(status)
	}
	status, _, _ = execute(t, h, "GET", "/api/v1/quotes/"+id, nil, cookie, csrf, s.Core.Config.Origin)
	if status != 200 {
		t.Fatal(status)
	}
}
func TestQuotesHumanStringsServiceMCPDeniedAndDisabledModule(t *testing.T) {
	s, a, r := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	if _, e := s.Core.DB.Exec(`ALTER TABLE quotes AUTO_INCREMENT=9007199254740993`); e != nil {
		t.Fatal(e)
	}
	id := makeQuote(t, s, a, "quote-big-id", quoteInput())
	if id != "9007199254740993" {
		t.Fatal(id)
	}
	token := deniedLegacyBusinessToken(t, s.Core, a, "cotizaciones.view")
	server := httptest.NewServer(api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil))))
	defer server.Close()
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	session, e := client.Connect(ctx, &mcp.StreamableClientTransport{Endpoint: server.URL + "/mcp", HTTPClient: &http.Client{Transport: bearer{token}}}, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer session.Close()
	result, e := session.CallTool(ctx, &mcp.CallToolParams{Name: "get_quote", Arguments: map[string]any{"id": id}})
	requireMCPBusinessDenied(t, result, e)
	q, e := s.Get(ctx, a, id)
	if e != nil || q.ID != id || q.Version != 1 || q.Calculation.Total != "201.25" {
		t.Fatal(q, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='cotizaciones.view'`); e != nil {
		t.Fatal(e)
	}
	result, e = session.CallTool(ctx, &mcp.CallToolParams{Name: "get_quote", Arguments: map[string]any{"id": id}})
	if e == nil && !result.IsError {
		t.Fatal("revoked tool", result)
	}
	t.Setenv("BASTION_QUOTES_ENABLED", "false")
	disabled := registry.New()
	_ = disabled.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	if e = composition.Register(s.Core, disabled, slog.Default()); e != nil {
		t.Fatal(e)
	}
	tokenCookie, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, disabled, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	status, _, _ := execute(t, h, "GET", "/api/v1/quotes", nil, &http.Cookie{Name: "bastion_dev_session", Value: tokenCookie}, csrf, s.Core.Config.Origin)
	if status != 404 {
		t.Fatal(status)
	}
}

func grantProcess(t *testing.T, s *core.Service, a core.Actor) {
	t.Helper()
	r, e := s.DB.Exec(`INSERT INTO roles(name) VALUES(CONCAT('process-quote-',UUID()))`)
	if e != nil {
		t.Fatal(e)
	}
	id, e := r.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec(`INSERT INTO role_permissions VALUES(?,'procesos.view','all')`, id); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec(`INSERT INTO user_roles VALUES(?,?)`, a.UserID, id); e != nil {
		t.Fatal(e)
	}
}
