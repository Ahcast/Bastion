package integration

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/ai"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"
)

func aiGrant(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	r, e := s.DB.Exec(`INSERT INTO roles(name) VALUES(?)`, "ai-"+security.ID())
	if e != nil {
		t.Fatal(e)
	}
	id, _ := r.LastInsertId()
	for _, p := range append(append([]string{}, ai.Permissions...), "procesos.view") {
		if _, e = s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?)`, id, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e = s.DB.Exec(`INSERT INTO user_roles VALUES(?,?)`, a.UserID, id); e != nil {
		t.Fatal(e)
	}
}
func aiFixture(t *testing.T) (*ai.Service, core.Actor, *processes.Service, core.Actor, string) {
	t.Helper()
	p, machine := processFixture(t)
	a := admin(t, p.Core)
	aiGrant(t, p.Core, a, "all")
	b := processes.Batch{ID: "ai-input-0", Schema: "1.0", Processes: []processes.Observation{obs(time.Now().UTC().Add(-time.Minute), "Entidad sintética")}}
	if _, e := p.Import(ctx, machine, "HC-synthetic-test", b.ID, b); e != nil {
		t.Fatal(e)
	}
	var id string
	if e := p.Core.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM processes`).Scan(&id); e != nil {
		t.Fatal(e)
	}
	return &ai.Service{Core: p.Core, Processes: p}, a, p, machine, id
}
func aiCreate(t *testing.T, s *ai.Service, a core.Actor, id string) ai.Task {
	t.Helper()
	src, e := s.Processes.Snapshot(ctx, s.Core.DB, a, id)
	if e != nil {
		t.Fatal(e)
	}
	v, e := s.Create(ctx, a, security.ID(), ai.CreateInput{Schema: "1.0", ProcessID: id, InputVersion: src.Version, SkillVersion: "score-v1"})
	if e != nil {
		t.Fatal(e)
	}
	return v
}
func aiClaim(t *testing.T, s *ai.Service, a core.Actor) ai.Reservation {
	t.Helper()
	v, e := s.Claim(ctx, a, security.ID(), ai.ClaimInput{Schema: "1.0", Limit: 1})
	if e != nil || len(v.Data) != 1 {
		t.Fatal(v, e)
	}
	return v.Data[0]
}
func aiEnvelope(r ai.Reservation) ai.ExecutionInput {
	return ai.ExecutionInput{Schema: "1.0", Version: r.Task.Version, RunID: r.RunID, InputVersion: r.Task.InputVersion, InputHash: r.Task.InputHash, SkillVersion: r.Task.SkillVersion, Lease: r.Lease}
}
func TestAITasksConcurrentDedupeClaimAndSecureReplay(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_, e := s.Create(ctx, a, fmt.Sprintf("ai-create-%02d", i), ai.CreateInput{Schema: "1.0", ProcessID: id, InputVersion: 1, SkillVersion: "score-v1"})
			errs <- e
		}(i)
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ai_tasks`).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	b := customerActor(t, s.Core, "all")
	aiGrant(t, s.Core, b, "all")
	claims := make(chan ai.Claims, 2)
	fail := make(chan error, 2)
	for _, actor := range []core.Actor{a, b} {
		wg.Add(1)
		go func(actor core.Actor) {
			defer wg.Done()
			v, e := s.Claim(ctx, actor, security.ID(), ai.ClaimInput{Schema: "1.0", Limit: 1})
			claims <- v
			fail <- e
		}(actor)
	}
	wg.Wait()
	close(claims)
	close(fail)
	for e := range fail {
		if e != nil {
			t.Fatal(e)
		}
	}
	n := 0
	for v := range claims {
		n += len(v.Data)
	}
	if n != 1 {
		t.Fatal("duplicate reservation", n)
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ai_runs`).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
}
func TestAITasksClaimReplayRenewFailCancelAndNoTokenDisclosure(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	key := security.ID()
	p := ai.ClaimInput{Schema: "1.0", Limit: 1}
	v, e := s.Claim(ctx, a, key, p)
	if e != nil || len(v.Data) != 1 {
		t.Fatal(v, e)
	}
	r := v.Data[0]
	for i := 0; i < 12; i++ {
		re, e := s.Claim(ctx, a, key, p)
		if e != nil || len(re.Data) != 1 || re.Data[0].Lease != r.Lease || re.Data[0].RunID != r.RunID {
			t.Fatal(re, e)
		}
	}
	p.Limit = 2
	if _, e = s.Claim(ctx, a, key, p); e != core.Conflict {
		t.Fatal("claim payload conflict", e)
	}
	d, e := s.Get(ctx, a, task.ID)
	if e != nil {
		t.Fatal(e)
	}
	raw, _ := json.Marshal(d)
	if strings.Contains(string(raw), r.Lease) || strings.Contains(string(raw), "lease_token") {
		t.Fatal("metadata token disclosure")
	}
	var response, cipher, hash string
	if e = s.Core.DB.QueryRow(`SELECT response FROM idempotency_records WHERE actor_key=? AND operation='ai.claim' AND request_key=?`, a.Key(), key).Scan(&response); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT lease_cipher,lease_hash FROM ai_runs WHERE id=?`, r.RunID).Scan(&cipher, &hash); e != nil {
		t.Fatal(e)
	}
	if strings.Contains(response, r.Lease) || cipher == r.Lease || hash != security.Hash(r.Lease) {
		t.Fatal("plaintext lease persisted")
	}
	env := aiEnvelope(r)
	rk := security.ID()
	renew, e := s.Renew(ctx, a, task.ID, rk, env)
	if e != nil || renew.Version != env.Version+1 {
		t.Fatal(renew, e)
	}
	if replay, e := s.Renew(ctx, a, task.ID, rk, env); e != nil || replay.Version != renew.Version {
		t.Fatal(replay, e)
	}
	env.Version = renew.Version
	renew, e = s.Renew(ctx, a, task.ID, security.ID(), env)
	if e != nil {
		t.Fatal(e)
	}
	env.Version = renew.Version
	if _, e = s.Renew(ctx, a, task.ID, security.ID(), env); e != core.Conflict {
		t.Fatal("renewal cap", e)
	}
	fp := ai.FailInput{ExecutionInput: env, Code: "executor_limit"}
	fk := security.ID()
	failed, e := s.Fail(ctx, a, task.ID, fk, fp)
	if e != nil || failed.Status != "pending" {
		t.Fatal(failed, e)
	}
	if re, e := s.Fail(ctx, a, task.ID, fk, fp); e != nil || re.Version != failed.Version {
		t.Fatal(re, e)
	}
	ck := security.ID()
	cancel, e := s.Cancel(ctx, a, task.ID, ck, ai.CancelInput{Schema: "1.0", Version: failed.Version})
	if e != nil || cancel.Status != "cancelled" {
		t.Fatal(cancel, e)
	}
	if _, e = s.Cancel(ctx, a, task.ID, ck, ai.CancelInput{Schema: "1.0", Version: failed.Version}); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Fail(ctx, a, task.ID, fk, fp); e != ai.LeaseExpired {
		t.Fatal("cancelled replay", e)
	}
	if re, e := s.Claim(ctx, a, key, ai.ClaimInput{Schema: "1.0", Limit: 1}); e != ai.LeaseExpired {
		t.Fatal("cancelled claim replay", re, e)
	}
	if again := aiCreate(t, s, a, id); again.ID != task.ID || again.Status != "cancelled" {
		t.Fatal("cancel creates duplicate", again)
	}
}
func TestAITasksLeaseRecoveryRejectsLateWritesAndExhaustion(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	var old ai.Reservation
	for i := 1; i <= 3; i++ {
		r := aiClaim(t, s, a)
		if r.Task.Attempts != i {
			t.Fatal(r)
		}
		if i > 1 {
			if _, e := s.Fail(ctx, a, task.ID, security.ID(), ai.FailInput{ExecutionInput: aiEnvelope(old), Code: "transient"}); e != ai.LeaseExpired {
				t.Fatal("replaced token", e)
			}
		}
		if _, e := s.Core.DB.Exec(`UPDATE ai_runs SET lease_expires_at=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE id=?`, r.RunID); e != nil {
			t.Fatal(e)
		}
		if _, e := s.Renew(ctx, a, task.ID, security.ID(), aiEnvelope(r)); e != ai.LeaseExpired {
			t.Fatal("expired renewal", e)
		}
		old = r
	}
	v, e := s.Claim(ctx, a, security.ID(), ai.ClaimInput{Schema: "1.0", Limit: 1})
	if e != nil || len(v.Data) != 0 {
		t.Fatal(v, e)
	}
	d, e := s.Get(ctx, a, task.ID)
	if e != nil || d.Status != "needs_review" || len(d.Runs) != 3 {
		t.Fatal(d, e)
	}
	for _, r := range d.Runs {
		if r.Status != "expired" {
			t.Fatal(r)
		}
	}
}
func TestAITasksTransientBackoffAndNonRetryableFailure(t *testing.T) {
	for _, code := range []string{"transient", "invalid_input"} {
		t.Run(code, func(t *testing.T) {
			s, a, _, _, id := aiFixture(t)
			task := aiCreate(t, s, a, id)
			for attempt := 1; attempt <= 3; attempt++ {
				r := aiClaim(t, s, a)
				v, e := s.Fail(ctx, a, task.ID, security.ID(), ai.FailInput{ExecutionInput: aiEnvelope(r), Code: code})
				if e != nil {
					t.Fatal(e)
				}
				if code == "invalid_input" {
					if v.Status != "needs_review" {
						t.Fatal(v)
					}
					break
				}
				if attempt == 3 {
					if v.Status != "failed" {
						t.Fatal(v)
					}
					break
				}
				if v.Status != "pending" || !v.Available.After(time.Now()) {
					t.Fatal("no backoff", v)
				}
				c, e := s.Claim(ctx, a, security.ID(), ai.ClaimInput{Schema: "1.0", Limit: 1})
				if e != nil || len(c.Data) != 0 {
					t.Fatal("retry before backoff", c, e)
				}
				if _, e = s.Core.DB.Exec(`UPDATE ai_tasks SET available_at=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE id=?`, task.ID); e != nil {
					t.Fatal(e)
				}
			}
		})
	}
}
func TestAITasksSourceChangedAndRevokedRequester(t *testing.T) {
	s, a, p, m, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	r := aiClaim(t, s, a)
	b := processes.Batch{ID: "ai-updated-0", Schema: "1.0", Processes: []processes.Observation{obs(time.Now().UTC(), "Entidad cambió")}}
	if _, e := p.Import(ctx, m, "HC-synthetic-test", b.ID, b); e != nil {
		t.Fatal(e)
	}
	if d, e := s.Get(ctx, a, task.ID); e != nil || d.CurrentInput || d.CurrentInputVersion != 2 {
		t.Fatal("stale input metadata", d, e)
	}
	if _, e := s.Fail(ctx, a, task.ID, security.ID(), ai.FailInput{ExecutionInput: aiEnvelope(r), Code: "transient"}); e != ai.InputStale {
		t.Fatal("stale source", e)
	}
	if _, e := s.Create(ctx, a, security.ID(), ai.CreateInput{Schema: "1.0", ProcessID: id, InputVersion: 2, SkillVersion: "score-v2"}); e != core.Conflict {
		t.Fatal("automatic revision", e)
	}
	if _, e := s.Cancel(ctx, a, task.ID, security.ID(), ai.CancelInput{Schema: "1.0", Version: r.Task.Version}); e != nil {
		t.Fatal("stale input cannot cancel", e)
	}
	// An independent source/task demonstrates a revoked original requester even
	// when the executor retains all of its own permissions.
	b.ID = "ai-independent-0"
	b.Processes[0] = obs(time.Now().UTC(), "Otra entidad")
	b.Processes[0].ExternalID = "official:independent"
	if _, e := p.Import(ctx, m, "HC-synthetic-test", b.ID, b); e != nil {
		t.Fatal(e)
	}
	var other string
	if e := s.Core.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM processes WHERE external_id='official:independent'`).Scan(&other); e != nil {
		t.Fatal(e)
	}
	aiCreate(t, s, a, other)
	executor := customerActor(t, s.Core, "all")
	aiGrant(t, s.Core, executor, "all")
	if _, e := s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='ai.tasks.create'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	v, e := s.Claim(ctx, executor, security.ID(), ai.ClaimInput{Schema: "1.0", Limit: 10})
	if e != nil || len(v.Data) != 0 {
		t.Fatal("revoked requester", v, e)
	}
}
func TestAITasksScopeServiceRevocationAndMalformedPayload(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	other := customerActor(t, s.Core, "self")
	aiGrant(t, s.Core, other, "self")
	l, e := s.List(ctx, other, ai.Filter{Page: 1, PerPage: 1})
	if e != nil || l.Total != 0 || len(l.Data) != 0 {
		t.Fatal("scope pagination", l, e)
	}
	if _, e = s.Get(ctx, other, task.ID); e != core.Missing {
		t.Fatal("foreign task", e)
	}
	v, e := s.Core.CreateService(ctx, a, "synthetic-ai", []string{"ai.tasks.read", "ai.tasks.execute", "procesos.view"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	token := v.(map[string]any)["token"].(string)
	executor, e := s.Core.AuthenticateService(ctx, token)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Create(ctx, executor, security.ID(), ai.CreateInput{Schema: "1.0", ProcessID: id, InputVersion: 1, SkillVersion: "score-v1"}); e != core.Forbidden {
		t.Fatal("service create", e)
	}
	reservation := aiClaim(t, s, executor)
	if e = s.Core.RevokeService(ctx, a, executor.ID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Renew(ctx, executor, task.ID, security.ID(), aiEnvelope(reservation)); e != core.Forbidden {
		t.Fatal("revoked cached actor", e)
	}
	read, e := s.Core.CreateService(ctx, a, "missing-source-scope", []string{"ai.tasks.read"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	actor, e := s.Core.AuthenticateService(ctx, read.(map[string]any)["token"].(string))
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.List(ctx, actor, ai.Filter{Page: 1, PerPage: 25}); e != core.Forbidden {
		t.Fatal("source scope omitted", e)
	}
	for _, input := range []ai.CreateInput{{Schema: "wrong", ProcessID: id, InputVersion: 1, SkillVersion: "v1"}, {Schema: "1.0", ProcessID: "0" + id, InputVersion: 1, SkillVersion: "v1"}, {Schema: "1.0", ProcessID: id, SkillVersion: "v1"}, {Schema: "1.0", ProcessID: id, InputVersion: 1, SkillVersion: "../../ bad"}} {
		if _, e = s.Create(ctx, a, security.ID(), input); e != core.Invalid {
			t.Fatal(input, e)
		}
	}
}
func TestAITasksAtomicRollback(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_ai_audit BEFORE INSERT ON audit_events FOR EACH ROW BEGIN IF NEW.action LIKE 'ai.%' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic failure'; END IF; END`); e != nil {
		t.Fatal(e)
	}
	key := security.ID()
	if _, e := s.Claim(ctx, a, key, ai.ClaimInput{Schema: "1.0", Limit: 1}); e == nil {
		t.Fatal("failed audit committed")
	}
	var runs, idem, events int
	for _, query := range []struct {
		sql string
		out *int
	}{{`SELECT COUNT(*) FROM ai_runs`, &runs}, {`SELECT COUNT(*) FROM idempotency_records WHERE operation='ai.claim'`, &idem}, {`SELECT COUNT(*) FROM domain_events WHERE topic='ai.run_started'`, &events}} {
		if e := s.Core.DB.QueryRow(query.sql).Scan(query.out); e != nil {
			t.Fatal(e)
		}
	}
	if runs != 0 || idem != 0 || events != 0 {
		t.Fatal(runs, idem, events)
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER reject_ai_audit`); e != nil {
		t.Fatal(e)
	}
	d, e := s.Get(ctx, a, task.ID)
	if e != nil || d.Status != "pending" || d.Version != 1 {
		t.Fatal(d, e)
	}
	aiClaim(t, s, a)
}
func TestAITasksRESTAndMCPRealSDKPolicies(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	reg := registry.New()
	_ = reg.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	_ = reg.Register(s.Module(slog.New(slog.NewTextHandler(io.Discard, nil)), true))
	handler := api.New(s.Core, reg, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	req := func(method, path, key string, body any, withCSRF bool) (int, map[string]any) {
		raw, _ := json.Marshal(body)
		r := httptest.NewRequest(method, path, strings.NewReader(string(raw)))
		r.RemoteAddr = "192.0.2.10:5000"
		r.AddCookie(cookie)
		r.Header.Set("Origin", s.Core.Config.Origin)
		r.Header.Set("Content-Type", "application/json")
		r.Header.Set("Idempotency-Key", key)
		if withCSRF {
			r.Header.Set("X-CSRF-Token", csrf)
		}
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, r)
		var result map[string]any
		_ = json.Unmarshal(w.Body.Bytes(), &result)
		return w.Code, result
	}
	input := ai.CreateInput{Schema: "1.0", ProcessID: id, InputVersion: 1, SkillVersion: "score-v1"}
	if code, _ := req("POST", "/api/v1/ai-tasks", security.ID(), input, false); code != 403 {
		t.Fatal("csrf", code)
	}
	if code, _ := req("POST", "/api/v1/ai-tasks", security.ID(), map[string]any{"schema_version": "1.0", "process_id": id, "input_version": "1", "skill_version": "v1", "requested_by": "1"}, true); code != 422 {
		t.Fatal("actor override", code)
	}
	code, created := req("POST", "/api/v1/ai-tasks", security.ID(), input, true)
	if code != 200 {
		t.Fatal(code, created)
	}
	taskID := created["id"].(string)
	v, e := s.Core.CreateService(ctx, a, "ai-sdk", []string{"procesos.view", "ai.tasks.read", "ai.tasks.execute"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	server := httptest.NewServer(handler)
	defer server.Close()
	client := mcp.NewClient(&mcp.Implementation{Name: "ai-synthetic", Version: "1"}, nil)
	c, cancel := context.WithTimeout(ctx, 15*time.Second)
	defer cancel()
	session, e := client.Connect(c, &mcp.StreamableClientTransport{Endpoint: server.URL + "/mcp", HTTPClient: &http.Client{Transport: bearer{v.(map[string]any)["token"].(string)}}}, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer session.Close()
	tools, e := session.ListTools(c, &mcp.ListToolsParams{})
	if e != nil {
		t.Fatal(e)
	}
	names := map[string]bool{}
	for _, tool := range tools.Tools {
		names[tool.Name] = true
	}
	for _, name := range []string{"list_ai_tasks", "get_ai_task", "claim_scoring_tasks", "renew_ai_task", "fail_ai_task"} {
		if !names[name] {
			t.Fatal("missing tool", name)
		}
	}
	result, e := session.CallTool(c, &mcp.CallToolParams{Name: "get_ai_task", Arguments: map[string]any{"task_id": taskID}})
	if e != nil || result.IsError {
		t.Fatal(result, e)
	}
	claimKey := security.ID()
	result, e = session.CallTool(c, &mcp.CallToolParams{Name: "claim_scoring_tasks", Arguments: map[string]any{"schema_version": "1.0", "limit": 1, "idempotency_key": claimKey}})
	if e != nil || result.IsError {
		t.Fatal(result, e)
	}
	raw, _ := json.Marshal(result.StructuredContent)
	var claimed ai.Claims
	if e = json.Unmarshal(raw, &claimed); e != nil || len(claimed.Data) != 1 {
		t.Fatal(string(raw), e)
	}
	envelope := aiEnvelope(claimed.Data[0])
	body, _ := json.Marshal(ai.FailInput{ExecutionInput: envelope, Code: "invalid_input"})
	args := map[string]any{}
	_ = json.Unmarshal(body, &args)
	args["task_id"] = taskID
	args["idempotency_key"] = security.ID()
	result, e = session.CallTool(c, &mcp.CallToolParams{Name: "fail_ai_task", Arguments: args})
	if e != nil || result.IsError {
		t.Fatal(result, e)
	}
	code, got := req("GET", "/api/v1/ai-tasks/"+taskID, "", nil, true)
	if code != 200 || got["status"] != "needs_review" {
		t.Fatal("REST/MCP diverged", code, got)
	}
	denied, e := session.CallTool(c, &mcp.CallToolParams{Name: "claim_scoring_tasks", Arguments: map[string]any{"schema_version": "wrong", "limit": 1, "idempotency_key": security.ID()}})
	if e == nil && !denied.IsError {
		t.Fatal("bad schema")
	}
	if code, _ := req("POST", "/api/v1/ai-tasks/"+taskID+"/complete", security.ID(), map[string]string{}, true); code != 404 {
		t.Fatal("generic completion exposed", code)
	}
}

func TestAITasksLiveTeamScopeAndLeaseActorBinding(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	other := customerActor(t, s.Core, "all")
	aiGrant(t, s.Core, other, "team")
	team, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo IA sintético')`)
	if e != nil {
		t.Fatal(e)
	}
	tid, e := team.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, tid, other.UserID); e != nil {
		t.Fatal(e)
	}
	if v, e := s.List(ctx, other, ai.Filter{Page: 1, PerPage: 25}); e != nil || v.Total != 0 {
		t.Fatal("foreign team", v, e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE processes SET team_id=? WHERE id=?`, tid, id); e != nil {
		t.Fatal(e)
	}
	if v, e := s.List(ctx, other, ai.Filter{Page: 1, PerPage: 25}); e != nil || v.Total != 1 {
		t.Fatal("live team unavailable", v, e)
	}
	r := aiClaim(t, s, other)
	if _, e = s.Renew(ctx, a, task.ID, security.ID(), aiEnvelope(r)); e != ai.LeaseExpired {
		t.Fatal("reservation stolen by another authorized actor", e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE user_id=?`, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Renew(ctx, other, task.ID, security.ID(), aiEnvelope(r)); e != core.Forbidden {
		t.Fatal("team revocation", e)
	}
}

func TestAITasksNewAttemptRejectsFailedOldReplay(t *testing.T) {
	s, a, _, _, id := aiFixture(t)
	task := aiCreate(t, s, a, id)
	old := aiClaim(t, s, a)
	first := ai.FailInput{ExecutionInput: aiEnvelope(old), Code: "transient"}
	key := security.ID()
	if _, e := s.Fail(ctx, a, task.ID, key, first); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE ai_tasks SET available_at=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE id=?`, task.ID); e != nil {
		t.Fatal(e)
	}
	newer := aiClaim(t, s, a)
	if _, e := s.Fail(ctx, a, task.ID, security.ID(), ai.FailInput{ExecutionInput: aiEnvelope(newer), Code: "invalid_input"}); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Fail(ctx, a, task.ID, key, first); e != ai.LeaseExpired {
		t.Fatal("prior attempt replay survived replacement", e)
	}
}
