package integration

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/composition"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func suppliersFixture(t *testing.T) (*suppliers.Service, core.Actor, *registry.Registry) {
	t.Helper()
	s := fixture(t)
	a := admin(t, s)
	r := registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Plataforma", Version: "0.1.0", Enabled: true})
	if e := composition.Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e != nil {
		t.Fatal(e)
	}
	return &suppliers.Service{Core: s}, a, r
}
func grantSupplier(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	rid := "suppliers-" + scope
	_, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, rid)
	if e != nil {
		t.Fatal(e)
	}
	var id string
	if e = s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, rid).Scan(&id); e != nil {
		t.Fatal(e)
	}
	for _, p := range suppliers.Permissions {
		if _, e = s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, id, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e = s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, id); e != nil {
		t.Fatal(e)
	}
}
func supplierActor(t *testing.T, s *core.Service, scope string) core.Actor {
	t.Helper()
	r, e := s.DB.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT CONCAT('supplier-',UUID(),'@example.test'),'Operador sintético',password_hash,totp_secret,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`)
	if e != nil {
		t.Fatal(e)
	}
	id, e := r.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	user := core.WrapID(id)
	token, _, e := s.NewSession(ctx, user, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	_, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, security.Hash(token))
	if e != nil {
		t.Fatal(e)
	}
	a := core.Actor{Type: "user", ID: user, UserID: user, SessionHash: security.Hash(token), Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
	grantSupplier(t, s, a, scope)
	return a
}
func supplierInput(name string) suppliers.Input {
	return suppliers.Input{Name: name, Country: "Honduras", Reliability: "unknown", Categories: []string{"Ferretería"}, Brands: []string{"Marca Sintética"}, Active: true, Contacts: []suppliers.Contact{{Name: "Contacto sintético", Role: "Compras", Email: "contact@example.test"}}, Products: []suppliers.Product{{Name: "Producto sintético", Brand: "Marca Sintética", PartNumber: "SKU-1", MOQ: "10.25", LeadTime: "7 días"}}}
}
func supplierResult(t *testing.T, v any, e error) map[string]string {
	t.Helper()
	if e != nil {
		t.Fatal(e)
	}
	b, e := json.Marshal(v)
	if e != nil {
		t.Fatal(e)
	}
	var out map[string]string
	if e = json.Unmarshal(b, &out); e != nil {
		t.Fatal(e)
	}
	return out
}
func makeSupplier(t *testing.T, s *suppliers.Service, a core.Actor, key string, p suppliers.Input) string {
	t.Helper()
	v, e := s.Save(ctx, a, "", key, p)
	return supplierResult(t, v, e)["id"]
}

func TestSuppliersPermissionsAreExplicitAndNoAutomaticBusinessGrant(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	if _, e := s.Save(ctx, a, "", "create-supplier", supplierInput("Proveedor")); e != core.Forbidden {
		t.Fatal("technical admin gained business grant", e)
	}
	v, e := s.Core.SaveRole(ctx, a, "", core.Role{Name: "Operador Proveedores", Grants: []core.Grant{{Permission: "proveedores.view", Scope: "team"}, {Permission: "proveedores.create", Scope: "self"}, {Permission: "proveedores.update", Scope: "all"}}})
	if e != nil || v == nil {
		t.Fatal("module permission unavailable in role administration", e)
	}
	if _, e = s.Core.CreateService(ctx, a, "ungranted", []string{"proveedores.view"}, time.Now().Add(time.Hour)); e != core.Forbidden {
		t.Fatal("service escalated owner's grants", e)
	}
	grantSupplier(t, s.Core, a, "all")
	if _, e = s.Core.CreateService(ctx, a, "write-denied", []string{"proveedores.update"}, time.Now().Add(time.Hour)); e != core.Forbidden {
		t.Fatal("service write scope", e)
	}
}
func TestSuppliersCRUDSnapshotFiltersNotesHistoryAndVersions(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	p := supplierInput("Institución Ejemplo")
	p.RTN = "0801-1990-123456"
	p.Email = "purchase@example.test"
	rating, response := 4, 24
	p.Evaluation = &rating
	p.ResponseHours = &response
	p.PaymentTerms = "Condiciones sintéticas"
	p.Notes = "Notas generales sintéticas"
	id := makeSupplier(t, s, a, "supplier-create", p)
	c, e := s.Get(ctx, a, id)
	if e != nil || len(c.Contacts) != 1 || len(c.Products) != 1 || c.Products[0].MOQ != "10.2500" || len(c.Categories) != 1 || len(c.Brands) != 1 || c.Country != "Honduras" || c.Evaluation == nil || *c.Evaluation != 4 || c.ResponseHours == nil || *c.ResponseHours != 24 || c.PaymentTerms != "Condiciones sintéticas" || c.Notes != "Notas generales sintéticas" || c.Version != 1 {
		t.Fatal(c, e)
	}
	snap, e := s.Snapshot(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	match, e := s.ResolveExact(ctx, a, "institucion  ejemplo", "")
	if e != nil || match == nil || (match.ID == nil || *match.ID != id) {
		t.Fatal(match, e)
	}
	match, e = s.ResolveExact(ctx, a, "wrong name", "08011990123456")
	if e != nil || match == nil || (match.ID == nil || *match.ID != id) {
		t.Fatal(match, e)
	}
	missing, e := s.ResolveExact(ctx, a, "Proveedor desconocido", "")
	if e != nil || missing != nil {
		t.Fatal("resolver created or invented supplier", e)
	}
	p = c.Input
	p.Name = "Institución Editada"
	p.ContactName = "Nuevo contacto"
	v, e := s.Save(ctx, a, id, "supplier-update", p)
	if supplierResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	if snap.Name != "Institución Ejemplo" || snap.Email != "purchase@example.test" {
		t.Fatal("snapshot mutated", snap)
	}
	if _, e = s.Save(ctx, a, id, "stale-update-key", p); e != core.Conflict {
		t.Fatal("lost update", e)
	}
	v, e = s.AddNote(ctx, a, id, "note-idempotent", "Nota sintética", 2)
	if supplierResult(t, v, e)["version"] != "3" {
		t.Fatal(v)
	}
	if _, e = s.AddNote(ctx, a, id, "note-idempotent", "Nota sintética", 2); e != nil {
		t.Fatal("note replay", e)
	}
	notes, e := s.Notes(ctx, a, id, 1, 1)
	if e != nil || notes.(map[string]any)["total"] != 1 {
		t.Fatal(notes, e)
	}
	h, e := s.History(ctx, a, id, 1, 2)
	if e != nil || h.(map[string]any)["total"] != 3 {
		t.Fatal(h, e)
	}
	for i := 0; i < 3; i++ {
		makeSupplier(t, s, a, fmt.Sprint("create-page-", i), func() suppliers.Input {
			p := supplierInput(fmt.Sprint("Otra institución ", i))
			p.Country = "China"
			return p
		}())
	}
	list, e := s.List(ctx, a, suppliers.Filter{Page: 2, PerPage: 2})
	if e != nil || list.Total != 4 || len(list.Data) != 2 {
		t.Fatal(list, e)
	}
	encoded, _ := json.Marshal(list.Data[0])
	var summary map[string]any
	_ = json.Unmarshal(encoded, &summary)
	if _, ok := summary["contacts"]; ok {
		t.Fatal("list misrepresented unloaded contacts as empty")
	}
	list, e = s.List(ctx, a, suppliers.Filter{Page: 1, PerPage: 5, Search: "institucion editada"})
	if e != nil || list.Total != 1 {
		t.Fatal(list, e)
	}
	list, e = s.List(ctx, a, suppliers.Filter{Page: 1, PerPage: 5, Search: "%"})
	if e != nil || list.Total != 0 {
		t.Fatal("LIKE wildcard or injection", list, e)
	}
	list, e = s.List(ctx, a, suppliers.Filter{Page: 1, PerPage: 5, Country: "honduras"})
	if e != nil || list.Total != 1 {
		t.Fatal(list, e)
	}
	c, e = s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p = c.Input
	p.Active = false
	if _, e = s.Save(ctx, a, id, "deactivate-supplier", p); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Snapshot(ctx, a, id); e != core.Conflict {
		t.Fatal("inactive usable for new documents", e)
	}
	active := true
	list, e = s.List(ctx, a, suppliers.Filter{Page: 1, PerPage: 5, Active: &active})
	if e != nil || list.Total != 3 {
		t.Fatal(list, e)
	}
	match, e = s.ResolveExact(ctx, a, "Institución Editada", "")
	if e != nil || match != nil {
		t.Fatal("inactive matched", match, e)
	}
}
func TestSuppliersConcurrentCanonicalAndRTNDuplicates(t *testing.T) {
	s, _, _ := suppliersFixture(t)
	actors := []core.Actor{}
	for i := 0; i < 8; i++ {
		actors = append(actors, supplierActor(t, s.Core, "all"))
	}
	var wg sync.WaitGroup
	var success, conflicts atomic.Int32
	for i, a := range actors {
		wg.Add(1)
		go func(i int, a core.Actor) {
			defer wg.Done()
			p := supplierInput("INSTITUCIÓN Ejemplo")
			if i%2 == 0 {
				p.Name = "  institucion   ejemplo  "
			}
			_, e := s.Save(ctx, a, "", fmt.Sprint("race-create-", i), p)
			if e == nil {
				success.Add(1)
			} else if e == core.Conflict {
				conflicts.Add(1)
			} else {
				t.Error(e)
			}
		}(i, a)
	}
	wg.Wait()
	if success.Load() != 1 || conflicts.Load() != 7 {
		t.Fatal(success.Load(), conflicts.Load())
	}
	var n int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events`).Scan(&n); e != nil || n != 1 {
		t.Fatal("duplicate event", n, e)
	}
	success.Store(0)
	conflicts.Store(0)
	for i, a := range actors {
		wg.Add(1)
		go func(i int, a core.Actor) {
			defer wg.Done()
			p := supplierInput(fmt.Sprint("Proveedor RTN ", i))
			p.RTN = "0801-1990-123456"
			_, e := s.Save(ctx, a, "", fmt.Sprint("rtn-create-", i), p)
			if e == nil {
				success.Add(1)
			} else if e == core.Conflict {
				conflicts.Add(1)
			} else {
				t.Error(e)
			}
		}(i, a)
	}
	wg.Wait()
	if success.Load() != 1 || conflicts.Load() != 7 {
		t.Fatal("RTN unique race", success.Load(), conflicts.Load())
	}
}
func TestSuppliersConcurrentIdempotencyAndRollback(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	var wg sync.WaitGroup
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, e := s.Save(ctx, a, "", "same-create-key", supplierInput("Proveedor único")); e != nil {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	for _, table := range []string{"suppliers", "domain_events", "supplier_contacts", "supplier_products"} {
		var n int
		if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ` + table).Scan(&n); e != nil || n != 1 {
			t.Fatal(table, n, e)
		}
	}
	if _, e := s.Save(ctx, a, "", "same-create-key", supplierInput("Otro payload")); e != core.Conflict {
		t.Fatal("idempotency payload mismatch", e)
	}
	// Real DB failure after aggregate/children writes must roll back everything.
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_supplier_audit_failure BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic audit outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Save(ctx, a, "", "rollback-create", supplierInput("Proveedor que falla")); e == nil {
		t.Fatal("audit failure swallowed")
	}
	for _, table := range []string{"suppliers", "domain_events", "supplier_contacts", "supplier_products", "idempotency_records"} {
		var n int
		if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ` + table).Scan(&n); e != nil || n != 1 {
			t.Fatal("rollback", table, n, e)
		}
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER test_supplier_audit_failure`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Save(ctx, a, "", "rollback-create", supplierInput("Proveedor que falla")); e != nil {
		t.Fatal("retry after rollback", e)
	}
	if e := s.Core.PublishOutbox(ctx); e != nil {
		t.Fatal(e)
	}
	var jobs int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM jobs`).Scan(&jobs); e != nil || jobs != 0 {
		t.Fatal("business events became unknown jobs", jobs, e)
	}
}
func TestSuppliersSelfTeamIDORAndImmediateRevocation(t *testing.T) {
	s, admin, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, admin, "all")
	owner := supplierActor(t, s.Core, "self")
	other := supplierActor(t, s.Core, "self")
	teamActor := supplierActor(t, s.Core, "team")
	if _, e := s.Core.DB.Exec(`INSERT INTO teams(id,name) VALUES(1,'Equipo uno'),(2,'Equipo dos')`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO team_members VALUES(1,?)`, teamActor.UserID); e != nil {
		t.Fatal(e)
	}
	foreignTeam := supplierInput("Equipo ajeno")
	foreignTeam.TeamID = "1"
	if _, e := s.Save(ctx, owner, "", "foreign-team-create", foreignTeam); e != core.Forbidden {
		t.Fatal("self scope assigned a foreign team", e)
	}
	ownID := makeSupplier(t, s, owner, "own-supplier", supplierInput("Propio"))
	p := supplierInput("Equipo")
	p.TeamID = "1"
	teamID := makeSupplier(t, s, admin, "team-supplier", p)
	for _, id := range []string{ownID, teamID} {
		if _, e := s.Get(ctx, other, id); e != core.Missing {
			t.Fatal("IDOR", e)
		}
	}
	list, e := s.List(ctx, other, suppliers.Filter{Page: 1, PerPage: 10})
	if e != nil || list.Total != 0 || len(list.Data) != 0 {
		t.Fatal("list leaked", list, e)
	}
	list, e = s.List(ctx, teamActor, suppliers.Filter{Page: 1, PerPage: 10})
	if e != nil || list.Total != 1 || list.Data[0].ID != teamID {
		t.Fatal("team list", list, e)
	}
	// A connection operator retains human team access without granting it to service tokens.
	r, e := s.Core.DB.Exec(`INSERT INTO roles(name) VALUES('Conexiones técnicas de prueba')`)
	if e != nil {
		t.Fatal(e)
	}
	rid, e := r.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO role_permissions VALUES(?,'core.services.manage','all')`, rid); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO user_roles VALUES(?,?)`, teamActor.UserID, rid); e != nil {
		t.Fatal(e)
	}
	// Technical connections do not inherit the operator's business team scope.
	token := deniedLegacyBusinessToken(t, s.Core, teamActor, "proveedores.view")
	serviceActor, e := s.Core.AuthenticateService(ctx, token)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.List(ctx, serviceActor, suppliers.Filter{Page: 1, PerPage: 10}); e != core.Forbidden {
		t.Fatal("legacy service read team data", e)
	}
	c, e := s.Get(ctx, teamActor, teamID)
	if e != nil {
		t.Fatal(e)
	}
	c.Input.Name = "Equipo editado"
	if _, e = s.Save(ctx, teamActor, teamID, "team-edit", c.Input); e != nil {
		t.Fatal(e)
	}
	c, e = s.Get(ctx, teamActor, teamID)
	if e != nil {
		t.Fatal(e)
	}
	c.Input.TeamID = "2"
	if _, e = s.Save(ctx, teamActor, teamID, "team-transfer", c.Input); e != core.Forbidden {
		t.Fatal("team scope expanded", e)
	}
	p = supplierInput("Equipo propio")
	p.TeamID = "1"
	if _, e = s.Save(ctx, teamActor, "", "team-create", p); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM team_members WHERE user_id=?`, teamActor.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Get(ctx, teamActor, teamID); e != core.Forbidden {
		t.Fatal("membership revocation", e)
	}
	if _, e = s.List(ctx, serviceActor, suppliers.Filter{Page: 1, PerPage: 10}); e != core.Forbidden {
		t.Fatal("service retained revoked team access", e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM user_roles WHERE user_id=?`, owner.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Get(ctx, owner, ownID); e != core.Forbidden {
		t.Fatal("role revocation", e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE users SET active=FALSE WHERE id=?`, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.List(ctx, other, suppliers.Filter{Page: 1, PerPage: 10}); e != core.Forbidden {
		t.Fatal("inactive identity", e)
	}
}
func TestSuppliersReauthAndOptimisticConcurrentUpdates(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	id := makeSupplier(t, s, a, "initial-supplier", supplierInput("Proveedor"))
	c, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	var success, conflict atomic.Int32
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			p := c.Input
			p.Name = fmt.Sprint("Proveedor versión ", i)
			_, e := s.Save(ctx, a, id, fmt.Sprint("supplier-edit-", i), p)
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
		t.Fatal("lost update", success.Load(), conflict.Load())
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 10 MINUTE)`); e != nil {
		t.Fatal(e)
	}
	c, e = s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := c.Input
	p.Active = false
	if _, e = s.Save(ctx, a, id, "sensitive-inactive", p); e == nil || !core.IsClient(e) {
		t.Fatal("no reauth required", e)
	}
	c, e = s.Get(ctx, a, id)
	if e != nil || !c.Active {
		t.Fatal("denied mutation applied", c, e)
	}
}
func TestSuppliersDocumentLinkRequiresBothPoliciesAndKeepsQuarantine(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	id := makeSupplier(t, s, a, "linked-supplier", supplierInput("Proveedor documento"))
	content := "%PDF-1.7\nsynthetic"
	hash := security.Hash(content)
	raw, _ := hexBytes(hash)
	fake := objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64.StdEncoding.EncodeToString(raw), Prefix: []byte(content)}}
	file, e := s.Core.CreateFile(ctx, a, "", "supplier-file", core.FileRequest{Filename: "test.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: hash}, fake)
	if e != nil {
		t.Fatal(e)
	}
	document := file.(map[string]any)["id"].(string)
	other := supplierActor(t, s.Core, "all")
	if _, e = s.LinkDocument(ctx, other, id, "no-file-grant", document, 1); e != core.Missing {
		t.Fatal("supplier grant bypassed file policy", e)
	}
	if _, e = s.LinkDocument(ctx, a, id, "document-link", document, 1); e != nil {
		t.Fatal(e)
	}
	if _, e = s.LinkDocument(ctx, a, id, "document-link", document, 1); e != nil {
		t.Fatal("link replay", e)
	}
	if e = s.Core.FinalizeFile(ctx, a, document, fake); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, document, 0, fake); e != core.Forbidden {
		t.Fatal("quarantine bypass", e)
	}
	if e = s.Core.ScanFile(ctx, a, document, 1, true, "synthetic-external-scan"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, document, 0, fake); e != nil {
		t.Fatal(e)
	}
	docs, e := s.Documents(ctx, a, id, 1, 25)
	if e != nil || docs.(map[string]any)["total"] != 1 {
		t.Fatal(docs, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='proveedores.view'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, document, 0, fake); e != core.Missing {
		t.Fatal("entity permission bypass", e)
	}
}
func TestSuppliersRESTAllowedServiceMCPDeniedAndDisabledModule(t *testing.T) {
	s, a, r := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	if _, e := s.Core.DB.Exec(`ALTER TABLE suppliers AUTO_INCREMENT=9007199254740993`); e != nil {
		t.Fatal(e)
	}
	id := makeSupplier(t, s, a, "mcp-supplier", supplierInput("Proveedor SDK"))
	if id != "9007199254740993" {
		t.Fatal("BIGINT precision", id)
	}
	token := deniedLegacyBusinessToken(t, s.Core, a, "proveedores.view")
	h := api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	server := httptest.NewServer(h)
	defer server.Close()
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	session, e := client.Connect(ctx, &mcp.StreamableClientTransport{Endpoint: server.URL + "/mcp", HTTPClient: &http.Client{Transport: bearer{token}}}, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer session.Close()
	result, e := session.CallTool(ctx, &mcp.CallToolParams{Name: "get_supplier", Arguments: map[string]any{"id": id}})
	requireMCPBusinessDenied(t, result, e)
	result, e = session.CallTool(ctx, &mcp.CallToolParams{Name: "search_suppliers", Arguments: map[string]any{"page": 1, "per_page": 1}})
	requireMCPBusinessDenied(t, result, e)
	// Human reads retain the exact ID and canonical name.
	out, e := s.Get(ctx, a, id)
	if e != nil || out.ID != id || out.Name != "Proveedor SDK" {
		t.Fatal(out, e)
	}
	actor, e := s.Core.AuthenticateService(ctx, token)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, actor, "", "machine-create", supplierInput("Machine")); e != core.Forbidden {
		t.Fatal("machine write", e)
	}
	cookieToken, csrf, e := s.Core.NewSession(ctx, "1", true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: cookieToken}
	status, response, _ := execute(t, h, "GET", "/api/v1/suppliers/"+id, nil, cookie, csrf, s.Core.Config.Origin)
	if status != 200 || response["id"] != out.ID {
		t.Fatal(status, response)
	}
	status, _, _ = execute(t, h, "PATCH", "/api/v1/suppliers/"+id, map[string]any{"created_by": "2"}, cookie, csrf, s.Core.Config.Origin)
	if status != 422 {
		t.Fatal("mass assignment", status)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='proveedores.view'`); e != nil {
		t.Fatal(e)
	}
	result, e = session.CallTool(ctx, &mcp.CallToolParams{Name: "get_supplier", Arguments: map[string]any{"id": id}})
	if e == nil && !result.IsError {
		t.Fatal("MCP revoked permission", result)
	}
	status, _, _ = execute(t, h, "GET", "/api/v1/suppliers/"+id, nil, cookie, csrf, s.Core.Config.Origin)
	if status != 403 {
		t.Fatal("REST revoked permission", status)
	}
	t.Setenv("BASTION_SUPPLIERS_ENABLED", "false")
	disabled := registry.New()
	_ = disabled.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	if e = composition.Register(s.Core, disabled, slog.Default()); e != nil {
		t.Fatal(e)
	}
	handler := api.New(s.Core, disabled, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	status, _, _ = execute(t, handler, "GET", "/api/v1/suppliers", nil, cookie, csrf, s.Core.Config.Origin)
	if status != 404 {
		t.Fatal("disabled route mounted", status)
	}
	// Independent session/context cannot be forged via caller headers.
	forged := httptest.NewRequest("GET", "/api/v1/suppliers", bytes.NewReader(nil))
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, forged)
	if w.Code != 401 {
		t.Fatal("anonymous access", w.Code)
	}
}
