package integration

import (
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/expenses"
)

func TestExpenseReferenceTwelveConcurrentReplaysAndRevocation(t *testing.T) {
	s, _ := expenseFixture(t)
	a := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, a, "all")
	id := makeExpense(t, s, a, "expense-reference-concurrent-source", expenseInput())
	before, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	stub := &expenseReferenceStub{label: "Proceso concurrente", version: 11}
	expenses.ConfigureReferenceSources(s, expenses.ReferenceSources{Processes: stub})
	input := expenses.SetReferencesInput{Version: 1, Reason: "Replay concurrente revisado", References: []expenses.ReferenceInput{{EntityType: "processes", EntityID: "42", EntityVersion: 11}}}

	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := s.SetReferences(ctx, a, id, "expense-reference-concurrent", input)
			errs <- err
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Fatal(err)
		}
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Version != 2 || after.Calculation != before.Calculation {
		t.Fatal(after, e)
	}
	refs, e := s.References(ctx, a, id)
	if e != nil || len(refs) != 1 || refs[0].Label != "Proceso concurrente" || refs[0].EntityVersion != 11 {
		t.Fatal(refs, e)
	}
	if stub.count() != 12 {
		t.Fatal("each replay must reauthorize the live source", stub.count())
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN roles r ON r.id=rp.role_id WHERE r.name='expenses-all' AND rp.permission='gastos.references.write'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.SetReferences(ctx, a, id, "expense-reference-concurrent", input); e != core.Forbidden {
		t.Fatal("revoked reference replay accepted", e)
	}
}

func TestExpenseDocumentTwelveConcurrentReplaysAndRevocation(t *testing.T) {
	s, _ := expenseFixture(t)
	a := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, a, "all")
	grantExpenseDocuments(t, s.Core, a, "all")
	s.Core.DocumentPolicies["expenses"] = s.DocumentPolicy
	id := makeExpense(t, s, a, "expense-document-concurrent-source", expenseInput())
	before, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	documentID := insertExpenseDocument(t, s.Core, a, "comprobante-concurrente.pdf")
	input := expenses.LinkDocumentInput{Version: 1, DocumentID: documentID, DocumentVersion: 1}

	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := s.LinkDocument(ctx, a, id, "expense-document-concurrent", input)
			errs <- err
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Fatal(err)
		}
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Version != 2 || after.Calculation != before.Calculation {
		t.Fatal(after, e)
	}
	var links int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM expense_documents WHERE expense_id=? AND document_id=?`, id, documentID).Scan(&links); e != nil || links != 1 {
		t.Fatal(links, e)
	}
	var documentVersion uint64
	if e = s.Core.DB.QueryRow(`SELECT version FROM documents WHERE id=?`, documentID).Scan(&documentVersion); e != nil || documentVersion != 2 {
		t.Fatal(documentVersion, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN roles r ON r.id=rp.role_id WHERE r.name='expenses-all' AND rp.permission='gastos.documents.write'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.LinkDocument(ctx, a, id, "expense-document-concurrent", input); e != core.Forbidden {
		t.Fatal("revoked document replay accepted", e)
	}
}

func TestExpenseLinkHTTPStrictCSRFAndIdempotency(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	id := makeExpense(t, s, a, "expense-link-http-source", expenseInput())
	expenses.ConfigureReferenceSources(s, expenses.ReferenceSources{Processes: &expenseReferenceStub{label: "Proceso HTTP", version: 7}})

	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e := r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, security.Hash(token)); e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	path := "/api/v1/expenses/" + id + "/references"
	valid := map[string]any{"version": "1", "reason": "Referencia HTTP revisada", "references": []any{map[string]any{"entity_type": "processes", "entity_id": "42", "entity_version": "7"}}}

	status, out := purchaseOrderHTTP(t, h, "POST", path, valid, cookie, "", s.Core.Config.Origin, "expense-link-http-csrf")
	if status != 403 {
		t.Fatal("missing CSRF", status, out)
	}
	unknown := map[string]any{"version": "1", "reason": "Referencia HTTP revisada", "references": []any{}, "unexpected": true}
	status, out = purchaseOrderHTTP(t, h, "POST", path, unknown, cookie, csrf, s.Core.Config.Origin, "expense-link-http-unknown")
	if status != 422 {
		t.Fatal("unknown field", status, out)
	}
	missing := map[string]any{"version": "1", "reason": "Referencia HTTP revisada"}
	status, out = purchaseOrderHTTP(t, h, "POST", path, missing, cookie, csrf, s.Core.Config.Origin, "expense-link-http-missing")
	if status != 422 {
		t.Fatal("missing references", status, out)
	}
	nullRefs := map[string]any{"version": "1", "reason": "Referencia HTTP revisada", "references": nil}
	status, out = purchaseOrderHTTP(t, h, "POST", path, nullRefs, cookie, csrf, s.Core.Config.Origin, "expense-link-http-null")
	if status != 422 {
		t.Fatal("null references", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, valid, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal("missing idempotency key", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, valid, cookie, csrf, s.Core.Config.Origin, "expense-link-http-valid")
	if status != 200 {
		t.Fatal("valid reference request", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", path+"?unknown=1", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal("reference query injection", status, out)
	}

	documentPath := "/api/v1/expenses/" + id + "/documents"
	documentUnknown := map[string]any{"version": "2", "document_id": "42", "document_version": "1", "unexpected": true}
	status, out = purchaseOrderHTTP(t, h, "POST", documentPath, documentUnknown, cookie, csrf, s.Core.Config.Origin, "expense-document-http-unknown")
	if status != 422 {
		t.Fatal("document unknown field", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", documentPath, map[string]any{"version": "2", "document_id": "42", "document_version": "1"}, cookie, "", s.Core.Config.Origin, "expense-document-http-csrf")
	if status != 403 {
		t.Fatal("document missing CSRF", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", documentPath+"?page=0&per_page=25", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal("document pagination", status, out)
	}
}
