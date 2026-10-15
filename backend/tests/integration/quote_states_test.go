package integration

import (
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"sync/atomic"
	"testing"
)

func TestQuotePreliminaryStatesRevisionsAndReplay(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "states-create", quoteInput())
	if _, e := s.Transition(ctx, a, id, "states-skip", quotes.TransitionInput{Status: "review", Version: 1}); e != core.Conflict {
		t.Fatal(e)
	}
	for _, state := range []string{"approved", "sent", "accepted", "rejected", "expired", ""} {
		if _, e := s.Transition(ctx, a, id, "states-illegal", quotes.TransitionInput{Status: state, Version: 1}); e != core.Invalid {
			t.Fatal(state, e)
		}
	}
	p := quotes.TransitionInput{Status: "costing", Version: 1}
	v, e := s.Transition(ctx, a, id, "states-costing", p)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	v, e = s.Transition(ctx, a, id, "states-costing", p)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	if _, e = s.Transition(ctx, a, id, "states-stale", p); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, id, "states-trash", 2, false); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Transition(ctx, a, id, "states-review", quotes.TransitionInput{Status: "review", Version: 2}); e != nil {
		t.Fatal(e)
	}
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Status != "review" || q.Version != 3 || len(q.Items) != 1 || q.Calculation.Total != "201.25" {
		t.Fatal(q, e)
	}
	if e = s.DocumentPolicy(ctx, s.Core.DB, a, id, true); e != core.Missing {
		t.Fatal("review files writable", e)
	}
	edited := editQuote(q)
	edited.Terms = "Nueva revisión"
	if _, e = s.Save(ctx, a, id, "states-edit", edited); e != nil {
		t.Fatal(e)
	}
	q, e = s.Get(ctx, a, id)
	if e != nil || q.Status != "draft" || q.Version != 4 || q.Number != "COT-000001" || q.Terms != edited.Terms {
		t.Fatal(q, e)
	}
	var raw string
	if e = s.Core.DB.QueryRow(`SELECT data FROM quote_versions WHERE quote_id=? AND version=3`, id).Scan(&raw); e != nil {
		t.Fatal(e)
	}
	var previous quotes.Quote
	if e = json.Unmarshal([]byte(raw), &previous); e != nil || previous.Status != "review" || previous.Terms == edited.Terms {
		t.Fatal(raw, e)
	}
	var count int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events WHERE entity_id=? AND topic='quote.revised'`, id).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	empty := quoteInput()
	empty.Items = nil
	empty.Discount = "0"
	emptyID := makeQuote(t, s, a, "states-empty", empty)
	if _, e = s.Transition(ctx, a, emptyID, "states-empty-costing", p); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Transition(ctx, a, emptyID, "states-empty-review", quotes.TransitionInput{Status: "review", Version: 2}); e != core.Invalid {
		t.Fatal(e)
	}
}

func TestQuoteStateConcurrentIntentsAndAuditRollback(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "state-race-create", quoteInput())
	var wg sync.WaitGroup
	var winners atomic.Int32
	actors := make([]core.Actor, 8)
	for i := range actors {
		actors[i] = customerActor(t, s.Core, "self")
		grantQuote(t, s.Core, actors[i], "all")
	}
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_, e := s.Transition(ctx, actors[i], id, "state-race-"+string(rune('a'+i)), quotes.TransitionInput{Status: "costing", Version: 1})
			if e == nil {
				winners.Add(1)
			} else if e != core.Conflict {
				t.Error(e)
			}
		}(i)
	}
	wg.Wait()
	if winners.Load() != 1 {
		t.Fatal(winners.Load())
	}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_state_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
		t.Fatal(e)
	}
	p := quotes.TransitionInput{Status: "review", Version: 2}
	if _, e := s.Transition(ctx, a, id, "state-rollback", p); e == nil {
		t.Fatal("audit failure ignored")
	}
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Status != "costing" || q.Version != 2 {
		t.Fatal(q, e)
	}
	var versions, events, intents int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quote_versions WHERE quote_id=?`, id).Scan(&versions); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events WHERE entity_id=? AND topic='quote.status_changed'`, id).Scan(&events); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM idempotency_records WHERE request_key='state-rollback'`).Scan(&intents); e != nil {
		t.Fatal(e)
	}
	if versions != 2 || events != 1 || intents != 0 {
		t.Fatal(versions, events, intents)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER test_state_audit`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Transition(ctx, a, id, "state-rollback", p); e != nil {
		t.Fatal(e)
	}
}

func TestQuoteStateLiveScopesRevocationAndREST(t *testing.T) {
	s, a, r := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	owner := customerActor(t, s.Core, "self")
	other := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, owner, "self")
	grantQuote(t, s.Core, other, "self")
	id := makeQuote(t, s, owner, "state-scope-create", quoteInput())
	p := quotes.TransitionInput{Status: "costing", Version: 1}
	if _, e := s.Transition(ctx, other, id, "state-idor", p); e != core.Missing {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, owner, id, "state-self", p); e != nil {
		t.Fatal(e)
	}
	team := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, team, "team")
	if _, e := s.Core.DB.Exec(`INSERT INTO teams(id,name) VALUES(1,'Equipo sintético')`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO team_members VALUES(1,?)`, team.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE quotes SET team_id=1 WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, team, id, "state-team", quotes.TransitionInput{Status: "review", Version: 2}); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM user_roles WHERE user_id=?`, owner.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Transition(ctx, owner, id, "state-self", p); e != core.Forbidden {
		t.Fatal("cached replay bypass", e)
	}
	machine := a
	machine.Type = "service"
	if _, e := s.Transition(ctx, machine, id, "state-machine", p); e != core.Forbidden {
		t.Fatal(e)
	}
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	for _, body := range []map[string]any{{"version": "3"}, {"status": "draft", "version": 3}, {"status": "draft", "version": "3", "created_by": "1"}, {"status": "sent", "version": "3"}} {
		status, _, _ := execute(t, h, "PATCH", "/api/v1/quotes/"+id+"/status", body, cookie, csrf, s.Core.Config.Origin)
		if status != 422 {
			t.Fatal(body, status)
		}
	}
}
