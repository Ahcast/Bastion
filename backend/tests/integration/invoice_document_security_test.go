package integration

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
)

type blockingInvoiceDocumentStore struct {
	started chan struct{}
	release chan struct{}
	once    sync.Once
}

func (s *blockingInvoiceDocumentStore) PutImmutable(context.Context, string, []byte, string) error {
	s.once.Do(func() { close(s.started) })
	<-s.release
	return nil
}

func TestInvoiceGeneratedDocumentReauthorizesBeforePublishingMetadata(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	store := &blockingInvoiceDocumentStore{started: make(chan struct{}), release: make(chan struct{})}
	configureInvoiceDocuments(t, s, store)
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Empresa revocación","tax_id":"08011900123456","address":"Tegucigalpa"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	value, e := s.Save(ctx, a, "", "document-revoke-create", invoiceInput())
	id := customerResult(t, value, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	type result struct {
		value any
		err   error
	}
	done := make(chan result, 1)
	go func() {
		value, e := s.GenerateDocument(ctx, a, id, "document-revoke-generate", invoices.GenerateDocumentInput{Kind: invoices.DocumentInvoice, Version: o.Version})
		done <- result{value: value, err: e}
	}()
	select {
	case <-store.started:
	case <-time.After(10 * time.Second):
		t.Fatal("generation never reached immutable storage")
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='facturas.documents.generate'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	close(store.release)
	select {
	case r := <-done:
		if r.err != core.Forbidden {
			t.Fatalf("finalization must fail after revocation: value=%v err=%v", r.value, r.err)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("generation did not finish after storage release")
	}
	var status string
	var documentID *string
	if e = s.Core.DB.QueryRow(`SELECT status,CAST(document_id AS CHAR) FROM invoice_document_snapshots WHERE invoice_id=?`, id).Scan(&status, &documentID); e != nil {
		t.Fatal(e)
	}
	if status != "pending" || documentID != nil {
		t.Fatalf("revoked finalization published metadata: status=%s document=%v", status, documentID)
	}
	var documents int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM documents WHERE entity_type='invoices' AND entity_id=?`, id).Scan(&documents); e != nil {
		t.Fatal(e)
	}
	if documents != 0 {
		t.Fatalf("revoked finalization registered %d Core documents", documents)
	}
}

func TestInvoiceGeneratedDocumentConcurrentReplayPublishesOnce(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	store := newInvoiceDocumentStore()
	configureInvoiceDocuments(t, s, store)
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Empresa concurrente","tax_id":"08011900123456","address":"Tegucigalpa"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	value, e := s.Save(ctx, a, "", "document-concurrent-create", invoiceInput())
	id := customerResult(t, value, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	input := invoices.GenerateDocumentInput{Kind: invoices.DocumentInvoice, Version: o.Version}
	type reply struct {
		value any
		err   error
	}
	replies := make(chan reply, 8)
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			value, e := s.GenerateDocument(ctx, a, id, "document-concurrent-generate", input)
			replies <- reply{value: value, err: e}
		}()
	}
	wg.Wait()
	close(replies)
	var snapshotID, documentID string
	for r := range replies {
		out := generatedDocumentResult(t, r.value, r.err)
		if out["status"] != "available" || out["id"] == "" || out["document_id"] == "" {
			t.Fatal(out)
		}
		if snapshotID == "" {
			snapshotID, documentID = out["id"], out["document_id"]
		} else if out["id"] != snapshotID || out["document_id"] != documentID {
			t.Fatalf("concurrent replay diverged: first=%s/%s got=%s/%s", snapshotID, documentID, out["id"], out["document_id"])
		}
	}
	var snapshots, documents int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM invoice_document_snapshots WHERE invoice_id=?`, id).Scan(&snapshots); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM documents WHERE entity_type='invoices' AND entity_id=?`, id).Scan(&documents); e != nil {
		t.Fatal(e)
	}
	objects, writes := store.stats()
	if snapshots != 1 || documents != 1 || objects != 1 || writes != 1 {
		t.Fatalf("expected one published artifact: snapshots=%d documents=%d objects=%d writes=%d", snapshots, documents, objects, writes)
	}
}
