package integration

import (
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"sync"
	"testing"
)

type pdfObjects struct {
	mu   sync.Mutex
	data map[string][]byte
	fail bool
	puts int
}

func (f *pdfObjects) PutImmutable(ctx context.Context, key string, b []byte, hash string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.puts++
	if f.fail {
		return errors.New("synthetic store outage")
	}
	if security.Hash(string(b)) != hash {
		return core.Invalid
	}
	if old, ok := f.data[key]; ok && !bytes.Equal(old, b) {
		return core.Conflict
	}
	f.data[key] = append([]byte{}, b...)
	return nil
}
func (f *pdfObjects) UploadURL(context.Context, string, string, int64, string) (string, error) {
	return "https://example.test/upload", nil
}
func (f *pdfObjects) DownloadURL(context.Context, string, string) (string, error) {
	return "https://example.test/authorized-download", nil
}
func (f *pdfObjects) Info(ctx context.Context, key string) (core.ObjectInfo, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	b := f.data[key]
	if b == nil {
		return core.ObjectInfo{}, core.Missing
	}
	raw, _ := hexBytes(security.Hash(string(b)))
	return core.ObjectInfo{Size: int64(len(b)), Checksum: base64.StdEncoding.EncodeToString(raw), Prefix: b[:min(512, len(b))]}, nil
}
func pdfFixture(t *testing.T) (*quotes.Service, core.Actor, *pdfObjects) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Value: []byte(`{"name":"Empresa sintética","tax_id":"08011900123456","address":"Dirección sintética"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	f := &pdfObjects{data: map[string][]byte{}}
	s.Store = f
	s.Core.DocumentPolicies["quotes"] = s.DocumentPolicy
	return s, a, f
}
func TestQuotePDFRecoveryFrozenPublicSnapshotAndImmutableMetadata(t *testing.T) {
	s, a, f := pdfFixture(t)
	id := makeQuote(t, s, a, "pdf-quote-create", quoteInput())
	f.fail = true
	if _, e := s.GeneratePDF(ctx, a, id, "pdf-same-intent", 1); e == nil {
		t.Fatal("store failure swallowed")
	}
	var raw, status string
	if e := s.Core.DB.QueryRow(`SELECT public_data,status FROM quote_pdf_snapshots`).Scan(&raw, &status); e != nil || status != "pending" || bytes.Contains([]byte(raw), []byte("Nota privada")) || bytes.Contains([]byte(raw), []byte("unit_cost")) {
		t.Fatal(raw, status, e)
	}
	q, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := editQuote(q)
	p.Customer.Name = "Cliente modificado"
	if _, e = s.Save(ctx, a, id, "pdf-edit-after-failure", p); e != nil {
		t.Fatal(e)
	}
	f.fail = false
	v, e := s.GeneratePDF(ctx, a, id, "pdf-same-intent", 1)
	r := customerResult(t, v, e)
	document := r["document_id"]
	if r["quote_version"] != "1" || r["status"] != "available" {
		t.Fatal(r)
	}
	if _, e = s.Core.DownloadFile(ctx, a, document, 0, f); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.CreateFile(ctx, a, document, "pdf-no-replace", core.FileRequest{Filename: "replace.pdf", MediaType: "application/pdf", Size: 4, Checksum: security.Hash("test"), Version: 1}, f); e != core.Conflict {
		t.Fatal("generated overwritten", e)
	}
	if _, e = s.Core.TrashFile(ctx, a, document, "pdf-no-delete", 1, false); e != core.Conflict {
		t.Fatal("generated deleted", e)
	}
	oldPuts := f.puts
	if _, e = s.GeneratePDF(ctx, a, id, "pdf-same-intent", 1); e != nil || f.puts != oldPuts {
		t.Fatal("available replay rerendered", e)
	}
	if _, e = s.GeneratePDF(ctx, a, id, "pdf-same-intent", 2); e != core.Conflict {
		t.Fatal("changed replay", e)
	}
	var n int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM documents`).Scan(&n); e != nil || n != 1 {
		t.Fatal(n, e)
	}
	if _, e = s.GeneratePDF(ctx, a, id, "pdf-new-version", 2); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM quote_pdf_snapshots`).Scan(&n); e != nil || n != 2 {
		t.Fatal(n, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='cotizaciones.pdf'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.GeneratePDF(ctx, a, id, "pdf-same-intent", 1); e != core.Missing {
		t.Fatal("replay permission bypass", e)
	}
}
func TestQuotePDFConcurrentIntentAndRollbackAfterObjectWrite(t *testing.T) {
	s, a, f := pdfFixture(t)
	id := makeQuote(t, s, a, "pdf-race-quote", quoteInput())
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, e := s.GeneratePDF(ctx, a, id, "pdf-concurrent-intent", 1); e != nil {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	for _, table := range []string{"quote_pdf_snapshots", "documents", "document_versions"} {
		var n int
		if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ` + table).Scan(&n); e != nil || n != 1 {
			t.Fatal(table, n, e)
		}
	}
	q, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := editQuote(q)
	p.Terms = "Nueva revisión"
	if _, e = s.Save(ctx, a, id, "pdf-next-revision", p); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER reject_generated_metadata BEFORE INSERT ON document_versions FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic metadata outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.GeneratePDF(ctx, a, id, "pdf-metadata-retry", 2); e == nil {
		t.Fatal("metadata failure swallowed")
	}
	var n int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM documents`).Scan(&n); e != nil || n != 1 {
		t.Fatal(n, e)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER reject_generated_metadata`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.GeneratePDF(ctx, a, id, "pdf-metadata-retry", 2); e != nil {
		t.Fatal("same immutable object failed recovery", e)
	}
	if len(f.data) != 2 {
		t.Fatal("duplicate objects", len(f.data))
	}
}
func TestQuoteAttachmentsBothPoliciesQuarantineTrashAndRestore(t *testing.T) {
	s, a, _ := pdfFixture(t)
	id := makeQuote(t, s, a, "file-quote-create", quoteInput())
	content := "%PDF-1.7\nsynthetic"
	raw, _ := hexBytes(security.Hash(content))
	f := objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64Checksum(raw), Prefix: []byte(content)}}
	v, e := s.Core.CreateFile(ctx, a, "", "quote-upload-file", core.FileRequest{Filename: "test.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, f)
	if e != nil {
		t.Fatal(e)
	}
	doc := v.(map[string]any)["id"].(string)
	other := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, other, "all")
	if _, e = s.LinkDocument(ctx, other, id, "quote-no-technical-grant", doc, 1, 1); e != core.Missing {
		t.Fatal(e)
	}
	v, e = s.LinkDocument(ctx, a, id, "quote-link-file", doc, 1, 1)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	if _, e = s.LinkDocument(ctx, a, id, "quote-link-file", doc, 1, 1); e != nil {
		t.Fatal("link replay", e)
	}
	if e = s.Core.FinalizeFile(ctx, a, doc, f); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != core.Forbidden {
		t.Fatal(e)
	}
	if e = s.Core.ScanFile(ctx, a, doc, 1, true, "synthetic-scan-reference"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.TrashFile(ctx, a, doc, "quote-trash-file", 2, false); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != core.Missing {
		t.Fatal("trash download", e)
	}
	docs, e := s.Documents(ctx, a, id, 1, 25, false)
	if e != nil || docs.(map[string]any)["total"] != 0 {
		t.Fatal(docs, e)
	}
	docs, e = s.Documents(ctx, a, id, 1, 25, true)
	if e != nil || docs.(map[string]any)["total"] != 1 {
		t.Fatal(docs, e)
	}
	if _, e = s.Core.TrashFile(ctx, a, doc, "quote-restore-file", 3, true); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != nil {
		t.Fatal(e)
	}
	delete(s.Core.DocumentPolicies, "quotes")
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != core.Forbidden {
		t.Fatal("disabled policy", e)
	}
	_ = fmt.Sprint(doc)
}
