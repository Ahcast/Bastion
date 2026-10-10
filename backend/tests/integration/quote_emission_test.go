package integration

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func emissionFixture(t *testing.T) (*quotes.Service, core.Actor, core.Actor, string, quotes.EmitInput) {
	t.Helper()
	s, a, reviewer, id := approvalFixture(t)
	v := requestApproval(t, s, a, id, "emission", 3)
	decideApproval(t, s, reviewer, id, v, 3, "approved")
	return s, a, reviewer, id, quotes.EmitInput{Version: 4, ApprovalID: v.ApprovalID}
}
func emissionCount(t *testing.T, s *quotes.Service, query string, want int) {
	t.Helper()
	var n int
	if e := s.Core.DB.QueryRow(query).Scan(&n); e != nil || n != want {
		t.Fatal(query, n, want, e)
	}
}
func TestQuoteEmissionSingleUseHistoricalReplayAndPrivacy(t *testing.T) {
	s, a, _, id, p := emissionFixture(t)
	f := s.Store.(*pdfObjects)
	draft, e := s.GeneratePDF(ctx, a, id, "emission-draft", 4)
	d := customerResult(t, draft, e)
	value, e := s.Emit(ctx, a, id, "emission-intention", p)
	r := customerResult(t, value, e)
	if r["kind"] != "issued" || r["quote_version"] != "4" || r["approval_id"] != p.ApprovalID || r["document_id"] == d["document_id"] {
		t.Fatal(r)
	}
	view := approvalView(t, s, a, id)
	if view.Status != "consumed" || view.Version != 3 {
		t.Fatal(view)
	}
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Status != "approved" || q.Version != 4 {
		t.Fatal(q, e)
	}
	emissionCount(t, s, `SELECT COUNT(*) FROM email_messages`, 0)
	emissionCount(t, s, `SELECT COUNT(*) FROM quote_versions`, 4)
	var raw string
	var issuedBy string
	if e = s.Core.DB.QueryRow(`SELECT public_data,issued_by FROM quote_pdf_snapshots WHERE kind='issued' AND issued_at IS NOT NULL`).Scan(&raw, &issuedBy); e != nil || issuedBy != a.UserID {
		t.Fatal(e, issuedBy)
	}
	for _, private := range []string{"Nota privada", "unit_cost", "margin", "created_by", "source_process_id", "input_hash"} {
		if strings.Contains(raw, private) {
			t.Fatal("public leak", private)
		}
	}
	if _, e = s.Core.DownloadFile(ctx, a, r["document_id"], 0, f); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.CreateFile(ctx, a, r["document_id"], "emit-no-replace", core.FileRequest{}, f); e == nil {
		t.Fatal("immutable file replaced")
	}
	edit := editQuote(q)
	edit.Terms = "Revisión posterior"
	if _, e = s.Save(ctx, a, id, "emission-edit", edit); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE approvals SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND)`); e != nil {
		t.Fatal(e)
	}
	puts := f.puts
	for _, key := range []string{"emission-intention", "emission-other-intent"} {
		value, e = s.Emit(ctx, a, id, key, p)
		again := customerResult(t, value, e)
		if again["id"] != r["id"] || f.puts != puts {
			t.Fatal(again, f.puts)
		}
	}
	if _, e = s.Emit(ctx, a, id, "emission-intention", quotes.EmitInput{Version: 5, ApprovalID: p.ApprovalID}); e != core.Conflict {
		t.Fatal("payload mismatch", e)
	}
	emissionCount(t, s, `SELECT COUNT(*) FROM quote_pdf_snapshots WHERE kind='issued'`, 1)
	emissionCount(t, s, `SELECT COUNT(*) FROM audit_events WHERE action='quote.issued'`, 1)
	emissionCount(t, s, `SELECT COUNT(*) FROM domain_events WHERE topic='quote.issued'`, 1)
	list, e := s.PDFs(ctx, a, id, 1, 25)
	if e != nil {
		t.Fatal(e)
	}
	b, _ := json.Marshal(list)
	if !strings.Contains(string(b), `"kind":"issued"`) || !strings.Contains(string(b), `"kind":"draft"`) {
		t.Fatal(string(b))
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='cotizaciones.emit'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Emit(ctx, a, id, "emission-intention", p); e != core.Missing {
		t.Fatal("cached unauthorized replay", e)
	}
}
func TestQuoteEmissionConcurrentIntentAndTransactionalRecovery(t *testing.T) {
	s, a, _, id, p := emissionFixture(t)
	f := s.Store.(*pdfObjects)
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_emission BEFORE INSERT ON audit_events FOR EACH ROW BEGIN IF NEW.action='quote.issued' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic emission failure'; END IF; END`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "emission-rollback", p); e == nil {
		t.Fatal("ignored failure")
	}
	if view := approvalView(t, s, a, id); view.Status != "approved" || !view.Valid {
		t.Fatal("consume not rolled back", view)
	}
	emissionCount(t, s, `SELECT COUNT(*) FROM documents`, 0)
	emissionCount(t, s, `SELECT COUNT(*) FROM quote_pdf_snapshots WHERE status='pending' AND issued_at IS NULL`, 1)
	if len(f.data) != 1 {
		t.Fatal("missing uploaded object")
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER reject_emission`); e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			key := "emission-rollback"
			if i >= 8 {
				key = fmt.Sprintf("different-emission-%d", i)
			}
			if _, e := s.Emit(ctx, a, id, key, p); e != nil {
				t.Error(e)
			}
		}(i)
	}
	wg.Wait()
	for _, table := range []string{"documents", "document_versions", "quote_pdf_snapshots"} {
		emissionCount(t, s, `SELECT COUNT(*) FROM `+table, 1)
	}
	emissionCount(t, s, `SELECT COUNT(*) FROM audit_events WHERE action='quote.issued'`, 1)
	emissionCount(t, s, `SELECT COUNT(*) FROM approval_events WHERE status='consumed'`, 1)
	if len(f.data) != 1 {
		t.Fatal("multiple objects", len(f.data))
	}
}

type emissionHook struct {
	store *pdfObjects
	after func()
}

func (f emissionHook) PutImmutable(c context.Context, k string, b []byte, h string) error {
	if e := f.store.PutImmutable(c, k, b, h); e != nil {
		return e
	}
	f.after()
	return nil
}
func TestQuoteEmissionRevalidationDuringObjectWrite(t *testing.T) {
	for _, change := range []string{"edit", "company", "revoke", "expiry", "approver_grant", "emitter_grant", "reauth", "files", "attachment"} {
		t.Run(change, func(t *testing.T) {
			s, a, reviewer, id, p := emissionFixture(t)
			store := s.Store.(*pdfObjects)
			s.Store = emissionHook{store, func() {
				var e error
				switch change {
				case "edit":
					q, err := s.Get(ctx, a, id)
					if err != nil {
						t.Fatal(err)
					}
					input := editQuote(q)
					input.Notes = "Cambiada durante subida"
					_, e = s.Save(ctx, a, id, "emission-during-edit", input)
				case "company":
					e = s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 1, Value: []byte(`{"name":"Empresa cambiada","tax_id":"08011900123456","address":"Dirección"}`)}, "")
				case "revoke":
					_, e = s.RevokeApproval(ctx, a, id, p.ApprovalID, "emission-during-revoke", quotes.ApprovalRevoke{QuoteVersion: 4, Version: 2, Reason: "Retirar"})
				case "expiry":
					_, e = s.Core.DB.Exec(`UPDATE approvals SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND)`)
				case "approver_grant":
					_, e = s.Core.DB.Exec(`DELETE FROM user_roles WHERE user_id=?`, reviewer.UserID)
				case "emitter_grant":
					_, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='cotizaciones.emit'`)
				case "reauth":
					_, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash)
				case "files":
					s.Core.Config.Files = false
				case "attachment":
					_, e = s.Core.DB.Exec(`INSERT INTO documents(owner_id,entity_type,entity_id,created_at) VALUES(?,'quotes',?,UTC_TIMESTAMP(6))`, a.UserID, id)
					if e == nil {
						_, e = s.Core.DB.Exec(`INSERT INTO document_versions SELECT MAX(id),1,'synthetic/new.pdf','evidence.pdf','application/pdf',4,REPEAT('a',64),'quarantined',NULL,UTC_TIMESTAMP(6) FROM documents`)
					}
				}
				if e != nil {
					t.Fatal(e)
				}
			}}
			if _, e := s.Emit(ctx, a, id, "emission-during-io", p); e == nil {
				t.Fatal("changed approval emitted")
			}
			emissionCount(t, s, `SELECT COUNT(*) FROM documents WHERE is_generated=TRUE`, 0)
			emissionCount(t, s, `SELECT COUNT(*) FROM quote_pdf_snapshots WHERE status='available'`, 0)
			emissionCount(t, s, `SELECT COUNT(*) FROM approval_events WHERE status='consumed'`, 0)
			var snapshot string
			if e := s.Core.DB.QueryRow(`SELECT id FROM quote_pdf_snapshots`).Scan(&snapshot); e != nil {
				t.Fatal(e)
			}
			if _, e := s.PDFDocument(ctx, a, id, snapshot); e == nil {
				t.Fatal("pending became downloadable")
			}
		})
	}
}
func TestQuoteEmissionStoreFailureReauthScopeAndServices(t *testing.T) {
	s, a, _, id, p := emissionFixture(t)
	store := s.Store.(*pdfObjects)
	stranger := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, stranger, "self")
	if _, e := s.Emit(ctx, stranger, id, "emission-idor", p); e != core.Missing {
		t.Fatal(e)
	}
	machine := a
	machine.Type = "service"
	if _, e := s.Emit(ctx, machine, id, "emission-machine", p); e != core.Forbidden {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "emission-reauth", p); e == nil || e.Error() != "reauth_required" {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	store.fail = true
	if _, e := s.Emit(ctx, a, id, "emission-store-retry", p); e == nil {
		t.Fatal("store error ignored")
	}
	if view := approvalView(t, s, a, id); !view.Valid {
		t.Fatal(view)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='core.documents.write'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "emission-store-retry", p); e != core.Forbidden {
		t.Fatal("document grant bypass", e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO role_permissions SELECT role_id,'core.documents.write','all' FROM user_roles WHERE user_id=? LIMIT 1`, a.UserID); e != nil {
		t.Fatal(e)
	}
	store.fail = false
	if _, e := s.Emit(ctx, a, id, "emission-store-retry", p); e != nil {
		t.Fatal(e)
	}
	s.Approvals = nil
	if _, e := s.Emit(ctx, a, id, "emission-store-retry", p); e != core.Disabled {
		t.Fatal(e)
	}
}
