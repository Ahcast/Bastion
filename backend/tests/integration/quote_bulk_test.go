package integration

import (
	"strings"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func TestQuoteBulkStatusIsTraceableAndIdempotent(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	first := makeQuote(t, s, a, "bulk-status-first", quoteInput())
	second := makeQuote(t, s, a, "bulk-status-second", quoteInput())
	input := quotes.BulkStatusInput{Status: "costing", Items: []quotes.BulkItemInput{{ID: first, Version: 1}, {ID: second, Version: 1}}}

	result, e := s.BulkStatus(ctx, a, "bulk-status-request", input)
	if e != nil {
		t.Fatal(e)
	}
	if result.BatchID == "" || result.Status != "completed" || len(result.Results) != 2 {
		t.Fatal(result)
	}
	for _, item := range result.Results {
		if item.Status != "success" || item.Error != "" {
			t.Fatal(item)
		}
	}

	replay, e := s.BulkStatus(ctx, a, "bulk-status-request", input)
	if e != nil || replay.BatchID != result.BatchID {
		t.Fatal(replay, e)
	}
	for _, id := range []string{first, second} {
		q, e := s.Get(ctx, a, id)
		if e != nil || q.Status != "costing" || q.Version != 2 {
			t.Fatal(q, e)
		}
	}
	var batches, items int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quote_batches`).Scan(&batches); e != nil || batches != 1 {
		t.Fatal("batches", batches, e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quote_batch_items`).Scan(&items); e != nil || items != 2 {
		t.Fatal("items", items, e)
	}

	changed := input
	changed.Status = "review"
	if _, e = s.BulkStatus(ctx, a, "bulk-status-request", changed); e != core.Conflict {
		t.Fatal("changed replay", e)
	}
}

func TestQuoteBulkCloneRecordsPartialFailureWithoutDuplicates(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "bulk-clone-source", quoteInput())
	input := quotes.BulkCloneInput{Items: []quotes.BulkItemInput{{ID: id, Version: 1}, {ID: "999999999", Version: 1}}}

	result, e := s.BulkClone(ctx, a, "bulk-clone-request", input)
	if e != nil {
		t.Fatal(e)
	}
	if result.Status != "completed" || len(result.Results) != 2 || result.Results[0].Status != "success" || result.Results[1].Status != "failed" || result.Results[1].Error != "not_found" {
		t.Fatal(result)
	}
	var count int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quotes`).Scan(&count); e != nil || count != 2 {
		t.Fatal(count, e)
	}

	replay, e := s.BulkClone(ctx, a, "bulk-clone-request", input)
	if e != nil || replay.BatchID != result.BatchID {
		t.Fatal(replay, e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quotes`).Scan(&count); e != nil || count != 2 {
		t.Fatal("clone duplicated", count, e)
	}
}

func TestQuoteBulkExportFreezesSafePublicSummary(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	input := quoteInput()
	input.Customer.Name = "=Cliente sintético"
	input.Reference = "+SUM(A1:A2)"
	id := makeQuote(t, s, a, "bulk-export-source", input)
	request := quotes.BulkExportInput{Items: []quotes.BulkItemInput{{ID: id, Version: 1}}}

	result, e := s.BulkExport(ctx, a, "bulk-export-request", request)
	if e != nil {
		t.Fatal(e)
	}
	if result.Status != "completed" || result.Filename != "cotizaciones-"+result.BatchID+".csv" || !strings.HasPrefix(result.Content, "\ufeff") {
		t.Fatal(result)
	}
	if !strings.Contains(result.Content, "'=Cliente sintético") || !strings.Contains(result.Content, "'+SUM(A1:A2)") {
		t.Fatal("spreadsheet formula not neutralized", result.Content)
	}
	if strings.Contains(result.Content, "Nota privada sintética") {
		t.Fatal("private notes leaked", result.Content)
	}
	frozen := result.Content

	q, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	edit := editQuote(q)
	edit.Customer.Name = "Cliente modificado después del lote"
	if _, e = s.Save(ctx, a, id, "bulk-export-edit", edit); e != nil {
		t.Fatal(e)
	}
	replay, e := s.BulkExport(ctx, a, "bulk-export-request", request)
	if e != nil || replay.BatchID != result.BatchID || replay.Content != frozen || strings.Contains(replay.Content, "Cliente modificado después del lote") {
		t.Fatal(replay, e)
	}
}

func TestQuoteBulkRejectsDuplicateSelection(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "bulk-duplicate-source", quoteInput())
	_, e := s.BulkClone(ctx, a, "bulk-duplicate-request", quotes.BulkCloneInput{Items: []quotes.BulkItemInput{{ID: id, Version: 1}, {ID: id, Version: 1}}})
	if e != core.Invalid {
		t.Fatal(e)
	}
	var batches int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quote_batches`).Scan(&batches); e != nil || batches != 0 {
		t.Fatal(batches, e)
	}
}
