package integration

import (
	"strings"
	"sync"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func TestQuoteBulkLedgerFailureRollsBackAndResumes(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "bulk-atomic-source", quoteInput())
	input := quotes.BulkCloneInput{Items: []quotes.BulkItemInput{{ID: id, Version: 1}}}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_bulk_ledger BEFORE UPDATE ON quote_batch_items FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic ledger outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.BulkClone(ctx, a, "bulk-atomic-request", input); e == nil {
		t.Fatal("ledger failure ignored")
	}
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quotes`).Scan(&count); e != nil || count != 1 {
		t.Fatal("effect escaped rollback", count, e)
	}
	var state string
	if e := s.Core.DB.QueryRow(`SELECT item_status FROM quote_batch_items`).Scan(&state); e != nil || state != "pending" {
		t.Fatal(state, e)
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER test_bulk_ledger`); e != nil {
		t.Fatal(e)
	}
	r, e := s.BulkClone(ctx, a, "bulk-atomic-request", input)
	if e != nil || r.Status != "completed" || r.Results[0].Status != "success" {
		t.Fatal(r, e)
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quotes`).Scan(&count); e != nil || count != 2 {
		t.Fatal(count, e)
	}
}

func TestQuoteBulkConcurrentReplayDoesNotDuplicate(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "bulk-race-source", quoteInput())
	input := quotes.BulkCloneInput{Items: []quotes.BulkItemInput{{ID: id, Version: 1}}}
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			r, e := s.BulkClone(ctx, a, "bulk-race-request", input)
			if e != nil || r.Status != "completed" || r.Results[0].Status != "success" {
				t.Error(r, e)
			}
		}()
	}
	wg.Wait()
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quotes`).Scan(&count); e != nil || count != 2 {
		t.Fatal(count, e)
	}
}

func TestQuoteBulkReplayRedactsLostRecordAccess(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "self")
	id := makeQuote(t, s, a, "bulk-scope-source", quoteInput())
	input := quotes.BulkExportInput{Items: []quotes.BulkItemInput{{ID: id, Version: 1}}}
	r, e := s.BulkExport(ctx, a, "bulk-scope-request", input)
	if e != nil || r.Results[0].Status != "success" {
		t.Fatal(r, e)
	}
	other := customerActor(t, s.Core, "self")
	if _, e := s.Core.DB.Exec(`UPDATE quotes SET created_by=? WHERE id=?`, other.UserID, id); e != nil {
		t.Fatal(e)
	}
	r, e = s.BulkExport(ctx, a, "bulk-scope-request", input)
	if e != nil || r.Results[0].Status != "unavailable" || len(r.Results[0].Result) != 0 || strings.Contains(r.Content, "COT-000001") {
		t.Fatal("cached export leaked", r, e)
	}
	var state string
	if e := s.Core.DB.QueryRow(`SELECT item_status FROM quote_batch_items`).Scan(&state); e != nil || state != "success" {
		t.Fatal("historical ledger altered", state, e)
	}
}
