package integration

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"sync"
	"testing"
)

func TestInvoiceClonePreservesHistoricalCommercialSnapshotWithoutFiscalLinks(t *testing.T) {
	s, a, id, emission := invoiceEmissionFixture(t)
	if _, e := s.Emit(ctx, a, id, "clone-source-emit", emission); e != nil {
		t.Fatal(e)
	}
	source, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	var counter uint64
	if e = s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&counter); e != nil {
		t.Fatal(e)
	}
	p := invoices.CloneInput{Version: source.Version, InvoiceDate: invoiceToday(), PublicNotes: "Nota nueva explícita"}
	v, e := s.Clone(ctx, a, id, "clone-source-once", p)
	first := customerResult(t, v, e)
	cloneID := first["id"]
	if cloneID == id || first["version"] != "1" {
		t.Fatal(first)
	}
	v, e = s.Clone(ctx, a, id, "clone-source-once", p)
	again := customerResult(t, v, e)
	if again["id"] != cloneID {
		t.Fatal(again, first)
	}
	clone, e := s.Get(ctx, a, cloneID)
	if e != nil || clone.Status != "draft" || clone.Number != nil || clone.SAR != nil || clone.Company != nil || clone.SourceQuoteID != nil || clone.SourcePurchaseOrderID != nil || clone.Customer != source.Customer || clone.CustomerVersion != source.CustomerVersion || clone.Calculation != source.Calculation || clone.PaidTotal != "0.00" || clone.Balance != source.Calculation.Total || clone.PublicNotes != p.PublicNotes || len(clone.Items) != len(source.Items) || clone.Items[0].ID == source.Items[0].ID {
		t.Fatal(clone, e)
	}
	var sourceID string
	var sourceVersion uint64
	if e = s.Core.DB.QueryRow(`SELECT CAST(source_invoice_id AS CHAR),source_version FROM invoice_clones WHERE invoice_id=?`, cloneID).Scan(&sourceID, &sourceVersion); e != nil || sourceID != id || sourceVersion != source.Version {
		t.Fatal(sourceID, sourceVersion, e)
	}
	var next uint64
	if e = s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&next); e != nil || next != counter {
		t.Fatal(counter, next, e)
	}
	var payments, documents int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM payments WHERE invoice_id=?`, cloneID).Scan(&payments); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM invoice_document_snapshots WHERE invoice_id=?`, cloneID).Scan(&documents); e != nil || payments != 0 || documents != 0 {
		t.Fatal(payments, documents, e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-source-stale", invoices.CloneInput{Version: source.Version + 1, InvoiceDate: p.InvoiceDate, PublicNotes: p.PublicNotes}); e != core.Conflict {
		t.Fatal(e)
	}
}

func TestInvoiceCancelledCannotBeCloned(t *testing.T) {
	s, a, id, emission := invoiceEmissionFixture(t)
	if _, e := s.Emit(ctx, a, id, "clone-cancel-emit", emission); e != nil {
		t.Fatal(e)
	}
	issued, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Cancel(ctx, a, id, "clone-cancel-action", invoices.CancelInput{Version: issued.Version, Reason: "Motivo", OriginalAndCopyMarkedAndArchived: true}); e != nil {
		t.Fatal(e)
	}
	current, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-cancel-reject", invoices.CloneInput{Version: current.Version, InvoiceDate: invoiceToday(), PublicNotes: ""}); e != core.Conflict {
		t.Fatal(e)
	}
}

func TestInvoiceCloneDraftAndConcurrentIndependentDestinations(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	p := invoiceInput()
	p.InvoiceDate = invoiceToday()
	v, e := s.Save(ctx, a, "", "clone-draft-source", p)
	id := customerResult(t, v, e)["id"]
	source, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	input := invoices.CloneInput{Version: source.Version, InvoiceDate: invoiceToday(), PublicNotes: "Copia de trabajo"}
	var wg sync.WaitGroup
	ids := make(chan string, 2)
	errs := make(chan error, 2)
	for _, key := range []string{"clone-draft-one", "clone-draft-two"} {
		wg.Add(1)
		go func(key string) {
			defer wg.Done()
			v, e := s.Clone(ctx, a, id, key, input)
			if e != nil {
				errs <- e
				return
			}
			ids <- v.(map[string]string)["id"]
		}(key)
	}
	wg.Wait()
	close(ids)
	close(errs)
	for e := range errs {
		t.Fatal(e)
	}
	seen := map[string]bool{}
	for cloneID := range ids {
		if seen[cloneID] || cloneID == id {
			t.Fatal(cloneID)
		}
		seen[cloneID] = true
		clone, e := s.Get(ctx, a, cloneID)
		if e != nil || clone.Status != "draft" || clone.Calculation != source.Calculation || clone.PublicNotes != input.PublicNotes || clone.Number != nil {
			t.Fatal(clone, e)
		}
	}
	if len(seen) != 2 {
		t.Fatal(seen)
	}
	current, e := s.Get(ctx, a, id)
	if e != nil || current.Version != source.Version {
		t.Fatal(current, e)
	}
}

func TestInvoiceCloneRollbackAndReplayRevocation(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	p := invoiceInput()
	p.InvoiceDate = invoiceToday()
	v, e := s.Save(ctx, a, "", "clone-auth-source", p)
	id := customerResult(t, v, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	input := invoices.CloneInput{Version: o.Version, InvoiceDate: invoiceToday(), PublicNotes: "Copia segura"}
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER invoice_clone_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic clone failure'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-rollback", input); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER invoice_clone_event_failure`); e != nil {
		t.Fatal(e)
	}
	var count int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM invoice_clones WHERE source_invoice_id=?`, id).Scan(&count); e != nil || count != 0 {
		t.Fatal(count, e)
	}
	v, e = s.Clone(ctx, a, id, "clone-replay-revoke", input)
	first := customerResult(t, v, e)
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='facturas.create'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-replay-revoke", input); e != core.Forbidden {
		t.Fatal(e)
	}
	if _, e = s.Clone(ctx, a, id, "clone-after-revoke", input); e != core.Forbidden {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM invoice_clones WHERE source_invoice_id=?`, id).Scan(&count); e != nil || count != 1 {
		t.Fatal(first, count, e)
	}
}
