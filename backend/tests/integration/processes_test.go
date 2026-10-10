package integration

import (
	"bytes"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/composition"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func processFixture(t *testing.T) (*processes.Service, core.Actor) {
	ss, a, _ := suppliersFixture(t)
	s := &processes.Service{Core: ss.Core}
	grantSupplier(t, s.Core, a, "all")
	var rid string
	s.Core.DB.QueryRow(`SELECT role_id FROM user_roles WHERE user_id=? ORDER BY role_id DESC LIMIT 1`, a.UserID).Scan(&rid)
	for _, p := range processes.Permissions {
		if _, e := s.Core.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,'all')`, rid, p); e != nil {
			t.Fatal(e)
		}
	}
	v, e := s.Core.CreateService(ctx, a, "crawler-lightsail", processes.CrawlerPermissions, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	m := v.(map[string]any)
	machine, e := s.Core.AuthenticateService(ctx, m["token"].(string))
	if e != nil {
		t.Fatal(e)
	}
	p := processes.RunInput{ID: "HC-synthetic-test", Started: time.Now().UTC().Add(-time.Hour).Truncate(time.Microsecond), Source: "honducompras"}
	if _, e = s.CreateRun(ctx, machine, p.ID, p); e != nil {
		t.Fatal(e)
	}
	return s, machine
}
func obs(at time.Time, entity string) processes.Observation {
	data := map[string]json.RawMessage{"entity": json.RawMessage(`"` + entity + `"`), "description": json.RawMessage(`"Texto sintético"`)}
	b, _ := json.Marshal(data)
	return processes.Observation{Source: "honducompras", ExternalID: "official:synthetic", URL: "https://example.test/process?id=1", Observed: at, Fields: []string{"entity", "description"}, Data: data, Completeness: "PARTIAL", ContentHash: security.Hash(string(b)), RawHash: security.Hash("raw")}
}
func TestProcessesImportReplayLateVersionAndRollback(t *testing.T) {
	s, a := processFixture(t)
	at := time.Now().UTC().Add(-time.Minute)
	p := processes.Batch{ID: "snapshot-one-0", Schema: "1.0", Processes: []processes.Observation{obs(at, "Entidad")}}
	v, e := s.Import(ctx, a, "HC-synthetic-test", p.ID, p)
	if e != nil || v.(processes.ACK).New != 1 {
		t.Fatal(v, e)
	}
	if _, e = s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e != nil {
		t.Fatal(e)
	}
	p.Processes[0] = obs(at, "Otra")
	if _, e = s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e != core.Conflict {
		t.Fatal("mismatch", e)
	}
	p.ID = "snapshot-two-0"
	p.Processes[0] = obs(at.Add(-time.Minute), "Atrasada")
	v, e = s.Import(ctx, a, "HC-synthetic-test", p.ID, p)
	if e != nil || v.(processes.ACK).Unchanged != 1 {
		t.Fatal(v, e)
	}
	var entity string
	var versions, count int
	s.Core.DB.QueryRow(`SELECT entity FROM processes`).Scan(&entity)
	s.Core.DB.QueryRow(`SELECT COUNT(*) FROM process_versions`).Scan(&versions)
	if entity != "Entidad" || versions != 1 {
		t.Fatal(entity, versions)
	}
	p.ID = "snapshot-three-0"
	p.Processes[0] = obs(at.Add(time.Second), "Nueva")
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER reject_process_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rejection'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e == nil {
		t.Fatal("rollback")
	}
	s.Core.DB.Exec(`DROP TRIGGER reject_process_audit`)
	s.Core.DB.QueryRow(`SELECT COUNT(*) FROM process_versions`).Scan(&versions)
	s.Core.DB.QueryRow(`SELECT COUNT(*) FROM crawler_deliveries`).Scan(&count)
	if versions != 1 || count != 2 {
		t.Fatal(versions, count)
	}
	if _, e = s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e != nil {
		t.Fatal(e)
	}
}
func TestProcessesConcurrentBatchAndRevocation(t *testing.T) {
	s, a := processFixture(t)
	p := processes.Batch{ID: "snapshot-one-0", Schema: "1.0", Processes: []processes.Observation{obs(time.Now().UTC().Add(-time.Minute), "Entidad")}}
	var wg sync.WaitGroup
	var failed atomic.Int32
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, e := s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e != nil {
				failed.Add(1)
			}
		}()
	}
	wg.Wait()
	var count int
	s.Core.DB.QueryRow(`SELECT COUNT(*) FROM process_observations`).Scan(&count)
	if failed.Load() != 0 || count != 1 {
		t.Fatal(failed.Load(), count)
	}
	s.Core.DB.Exec(`UPDATE service_identities SET active=FALSE WHERE id=?`, a.ServiceID)
	if _, e := s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e == nil {
		t.Fatal("revoked replay")
	}
}
func TestProcessesCompletionCountersPersonalAndExport(t *testing.T) {
	s, a := processFixture(t)
	p := processes.Batch{ID: "snapshot-one-0", Schema: "1.0", Processes: []processes.Observation{obs(time.Now().UTC().Add(-time.Minute), "=Entidad")}}
	v, e := s.Import(ctx, a, "HC-synthetic-test", p.ID, p)
	if e != nil {
		t.Fatal(e)
	}
	ack := v.(processes.ACK)
	ex := map[string]json.RawMessage{"run_id": json.RawMessage(`"HC-synthetic-test"`), "status": json.RawMessage(`"PARTIAL_SUCCESS"`), "records": json.RawMessage(`1`)}
	done := processes.Complete{ID: "snapshot-one", Extraction: ex, Import: ack}
	if _, e = s.Complete(ctx, a, "HC-synthetic-test", done.ID+"-complete", done); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Complete(ctx, a, "HC-synthetic-test", done.ID+"-complete", done); e != nil {
		t.Fatal(e)
	}
	done.Import.Received++
	if _, e = s.Complete(ctx, a, "HC-synthetic-test", done.ID+"-complete", done); e != core.Conflict {
		t.Fatal("counters", e)
	}
	human := admin(t, s.Core)
	var id string
	s.Core.DB.QueryRow(`SELECT id FROM processes`).Scan(&id)
	interest := true
	if _, e = s.Personal(ctx, human, id, "personal-interest-key", "mark", processes.PersonalInput{Interesting: &interest}); e != nil {
		t.Fatal(e)
	}
	note := "Nota personal"
	if _, e = s.Personal(ctx, human, id, "personal-note-key", "note", processes.PersonalInput{Note: &note, Version: 1}); e != nil {
		t.Fatal(e)
	}
	p.ID = "snapshot-two-0"
	p.Processes[0] = obs(time.Now().UTC().Add(-30*time.Second), "=Entidad nueva")
	if _, e = s.Import(ctx, a, "HC-synthetic-test", p.ID, p); e != nil {
		t.Fatal(e)
	}
	hist, e := s.Versions(ctx, human, id, 1, 25)
	if e != nil {
		t.Fatal(e)
	}
	vlist := hist.(map[string]any)["data"].([]map[string]any)
	diff := vlist[0]["diff"].([]processes.Change)
	if len(diff) != 1 || diff[0].Kind != "METADATA_CHANGED" || string(diff[0].Before) != `"=Entidad"` || string(diff[0].After) != `"=Entidad nueva"` {
		t.Fatal(diff)
	}
	record, e := s.Get(ctx, human, id)
	if e != nil || record.Note != note || !record.Interesting || record.PersonalVersion != 2 || record.Version != 2 {
		t.Fatal(record, e)
	}
	export, e := s.Export(ctx, human, processes.Filter{})
	if e != nil || !strings.Contains(export.(map[string]string)["content"], "'=Entidad nueva") {
		t.Fatal(export, e)
	}
	if _, e = s.Personal(ctx, a, id, "service-note-key", "note", processes.PersonalInput{Note: &note, Version: 2}); e != core.Invalid {
		t.Fatal("service write", e)
	}
}
func TestProcessesHTTPMCPAndScopedReadRevocation(t *testing.T) {
	s, a := processFixture(t)
	if _, e := s.Core.DB.Exec(`ALTER TABLE processes AUTO_INCREMENT=9007199254740993`); e != nil {
		t.Fatal(e)
	}
	token := "synthetic-crawler-token"
	s.Core.DB.Exec(`UPDATE service_identities SET token_hash=? WHERE id=?`, security.Hash(token), a.ServiceID)
	r := registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Plataforma", Enabled: true, Version: "0.1.0"})
	if e := composition.Register(s.Core, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e != nil {
		t.Fatal(e)
	}
	server := httptest.NewServer(api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil))))
	defer server.Close()
	p := processes.Batch{ID: "snapshot-one-0", Schema: "1.0", Processes: []processes.Observation{obs(time.Now().UTC().Add(-time.Minute), "Entidad")}}
	body, _ := json.Marshal(p)
	send := func(tok string) int {
		req, _ := http.NewRequest("POST", server.URL+"/api/internal/v1/crawler/runs/HC-synthetic-test/processes", bytes.NewReader(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Idempotency-Key", p.ID)
		if tok != "" {
			req.Header.Set("Authorization", "Bearer "+tok)
		}
		res, e := server.Client().Do(req)
		if e != nil {
			t.Fatal(e)
		}
		defer res.Body.Close()
		return res.StatusCode
	}
	if code := send(""); code != 401 {
		t.Fatal(code)
	}
	if code := send(token); code != 200 {
		t.Fatal(code)
	}
	human := admin(t, s.Core)
	v, e := s.Core.CreateService(ctx, human, "read-process", []string{"procesos.view"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	readToken := v.(map[string]any)["token"].(string)
	if code := send(readToken); code != 403 {
		t.Fatal("read token ingested", code)
	}
	var id string
	s.Core.DB.QueryRow(`SELECT id FROM processes`).Scan(&id)
	if id != "9007199254740993" {
		t.Fatal("BIGINT precision", id)
	}
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	session, e := client.Connect(ctx, &mcp.StreamableClientTransport{Endpoint: server.URL + "/mcp", HTTPClient: &http.Client{Transport: bearer{readToken}}}, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer session.Close()
	result, e := session.CallTool(ctx, &mcp.CallToolParams{Name: "get_process", Arguments: map[string]any{"id": id}})
	if e != nil || result.IsError {
		t.Fatal(result, e)
	}
	var rid string
	s.Core.DB.QueryRow(`SELECT role_id FROM user_roles WHERE user_id=? ORDER BY role_id DESC LIMIT 1`, human.UserID).Scan(&rid)
	s.Core.DB.Exec(`UPDATE role_permissions SET scope='self' WHERE role_id=? AND permission='procesos.view'`, rid)
	s.Core.DB.Exec(`UPDATE processes SET created_by=? WHERE id=?`, supplierActor(t, s.Core, "self").UserID, id)
	list, e := s.List(ctx, human, processes.Filter{Page: 1, PerPage: 25})
	if e != nil || list.Total != 0 {
		t.Fatal(list, e)
	}
	if _, e = s.Get(ctx, human, id); e != core.Missing {
		t.Fatal("IDOR", e)
	}
	s.Core.DB.Exec(`DELETE FROM role_permissions WHERE role_id=? AND permission='procesos.view'`, rid)
	result, e = session.CallTool(ctx, &mcp.CallToolParams{Name: "get_process", Arguments: map[string]any{"id": id}})
	if e == nil && !result.IsError {
		t.Fatal("MCP revoke")
	}
}
func TestProcessesLargeBatchWithoutGlobalPacketChanges(t *testing.T) {
	s, a := processFixture(t)
	p := processes.Batch{ID: "large-snapshot-0", Schema: "1.0", Processes: []processes.Observation{}}
	for i := 0; i < 30; i++ {
		o := obs(time.Now().UTC().Add(-time.Minute), "Entidad")
		o.ExternalID = fmt.Sprintf("synthetic-large-%d", i)
		b, _ := json.Marshal(strings.Repeat("a", 180000))
		o.Data["description"] = b
		d, _ := json.Marshal(o.Data)
		o.ContentHash = security.Hash(string(d))
		p.Processes = append(p.Processes, o)
	}
	raw, _ := json.Marshal(p)
	if len(raw) < 4*1024*1024 {
		t.Fatal("fixture must exceed default packet")
	}
	v, e := s.Import(ctx, a, "HC-synthetic-test", p.ID, p)
	if e != nil || v.(processes.ACK).New != 30 {
		t.Fatal(v, e)
	}
	var size, count int
	if e = s.Core.DB.QueryRow(`SELECT LENGTH(payload) FROM crawler_deliveries WHERE batch_id=?`, p.ID).Scan(&size); e != nil {
		t.Fatal(e)
	}
	s.Core.DB.QueryRow(`SELECT COUNT(*) FROM process_observations`).Scan(&count)
	if size > 1024 || count != 30 {
		t.Fatal(size, count)
	}
}
