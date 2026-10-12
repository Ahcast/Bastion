package integration

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
	"sync"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/expenses"
)

type expenseReferenceStub struct {
	mu      sync.Mutex
	label   string
	version uint64
	err     error
	calls   int
}

func (s *expenseReferenceStub) ExpenseReferenceInTransaction(_ context.Context, _ *sql.Tx, _ core.Actor, _ string) (string, uint64, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.calls++
	return s.label, s.version, s.err
}

func (s *expenseReferenceStub) set(label string, version uint64, err error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.label, s.version, s.err = label, version, err
}

func (s *expenseReferenceStub) count() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.calls
}

func grantExpenseDocuments(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	if _, e := s.DB.Exec(`INSERT IGNORE INTO permissions(name) VALUES('core.documents.read'),('core.documents.write')`); e != nil {
		t.Fatal(e)
	}
	name := "expense-documents-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var role string
	if e := s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, name).Scan(&role); e != nil {
		t.Fatal(e)
	}
	for _, permission := range []string{"core.documents.read", "core.documents.write"} {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, role, permission, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, role); e != nil {
		t.Fatal(e)
	}
}

func insertExpenseDocument(t *testing.T, s *core.Service, a core.Actor, filename string) string {
	t.Helper()
	r, e := s.DB.Exec(`INSERT INTO documents(owner_id,created_at) VALUES(?,UTC_TIMESTAMP(6))`, a.UserID)
	if e != nil {
		t.Fatal(e)
	}
	n, e := r.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	id := core.WrapID(n)
	if _, e = s.DB.Exec(`INSERT INTO document_versions(document_id,number,object_key,filename,media_type,size_bytes,checksum,status,scan_reference,created_at) VALUES(?,1,?,?,?,?,?,'available','synthetic-expense-document',UTC_TIMESTAMP(6))`, id, "synthetic/expense/"+id+"/1", filename, "application/pdf", 4, strings.Repeat("a", 64)); e != nil {
		t.Fatal(e)
	}
	return id
}

func TestExpenseReferencesSnapshotReplayVersionAndRollback(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	id := makeExpense(t, s, a, "expense-reference-source", expenseInput())
	stub := &expenseReferenceStub{label: "Proceso histórico", version: 7}
	expenses.ConfigureReferenceSources(s, expenses.ReferenceSources{Processes: stub})

	input := expenses.SetReferencesInput{Version: 1, Reason: "Vincular soporte revisado", References: []expenses.ReferenceInput{{EntityType: "processes", EntityID: "42", EntityVersion: 7}}}
	value, e := s.SetReferences(ctx, a, id, "expense-reference-set", input)
	if customerResult(t, value, e)["version"] != "2" {
		t.Fatal(value, e)
	}
	refs, e := s.References(ctx, a, id)
	if e != nil || len(refs) != 1 || refs[0].Label != "Proceso histórico" || refs[0].EntityVersion != 7 || refs[0].EntityID != "42" {
		t.Fatal(refs, e)
	}

	stub.set("Proceso cambiado", 8, nil)
	value, e = s.SetReferences(ctx, a, id, "expense-reference-set", input)
	if e != nil || customerResult(t, value, e)["version"] != "2" || stub.count() < 2 {
		t.Fatal(value, e, stub.count())
	}
	refs, e = s.References(ctx, a, id)
	if e != nil || refs[0].Label != "Proceso histórico" || refs[0].EntityVersion != 7 {
		t.Fatal(refs, e)
	}

	stale := expenses.SetReferencesInput{Version: 2, Reason: "Intento con versión antigua", References: []expenses.ReferenceInput{{EntityType: "processes", EntityID: "42", EntityVersion: 7}}}
	if _, e = s.SetReferences(ctx, a, id, "expense-reference-stale", stale); e != core.Conflict {
		t.Fatal(e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Version != 2 {
		t.Fatal(o, e)
	}

	expenses.ConfigureReferenceSources(s, expenses.ReferenceSources{})
	if _, e = s.SetReferences(ctx, a, id, "expense-reference-set", input); e != core.Disabled {
		t.Fatal(e)
	}
	expenses.ConfigureReferenceSources(s, expenses.ReferenceSources{Processes: stub})
	empty := expenses.SetReferencesInput{Version: 2, Reason: "Quitar referencia revisada", References: []expenses.ReferenceInput{}}
	value, e = s.SetReferences(ctx, a, id, "expense-reference-clear", empty)
	if e != nil || customerResult(t, value, e)["version"] != "3" {
		t.Fatal(value, e)
	}
	refs, e = s.References(ctx, a, id)
	if e != nil || len(refs) != 0 {
		t.Fatal(refs, e)
	}

	stub.set("Proceso para rollback", 9, nil)
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER expense_reference_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic expense reference failure'`); e != nil {
		t.Fatal(e)
	}
	rollback := expenses.SetReferencesInput{Version: 3, Reason: "Probar rollback", References: []expenses.ReferenceInput{{EntityType: "processes", EntityID: "42", EntityVersion: 9}}}
	if _, e = s.SetReferences(ctx, a, id, "expense-reference-rollback", rollback); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER expense_reference_event_failure`); e != nil {
		t.Fatal(e)
	}
	o, e = s.Get(ctx, a, id)
	refs, refsErr := s.References(ctx, a, id)
	if e != nil || refsErr != nil || o.Version != 3 || len(refs) != 0 {
		t.Fatal(o, refs, e, refsErr)
	}
}

func TestExpenseDocumentLinkReplayIsolationRollbackAndDeletionPolicy(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantExpenseDocuments(t, s.Core, a, "all")
	s.Core.DocumentPolicies["expenses"] = s.DocumentPolicy
	id := makeExpense(t, s, a, "expense-document-source", expenseInput())
	documentID := insertExpenseDocument(t, s.Core, a, "factura-proveedor.pdf")

	input := expenses.LinkDocumentInput{Version: 1, DocumentID: documentID, DocumentVersion: 1}
	value, e := s.LinkDocument(ctx, a, id, "expense-document-link", input)
	result := customerResult(t, value, e)
	if result["version"] != "2" || result["document_version"] != "2" {
		t.Fatal(result)
	}
	value, e = s.LinkDocument(ctx, a, id, "expense-document-link", input)
	if e != nil || customerResult(t, value, e)["version"] != "2" {
		t.Fatal(value, e)
	}
	var entityType, entityID string
	var documentVersion uint64
	if e = s.Core.DB.QueryRow(`SELECT entity_type,CAST(entity_id AS CHAR),version FROM documents WHERE id=?`, documentID).Scan(&entityType, &entityID, &documentVersion); e != nil {
		t.Fatal(e)
	}
	if entityType != "expenses" || entityID != id || documentVersion != 2 {
		t.Fatal(entityType, entityID, documentVersion)
	}
	page, e := s.Documents(ctx, a, id, 1, 25)
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != documentID || page.Data[0].Filename != "factura-proveedor.pdf" || page.Data[0].LinkedDocumentVersion != 2 {
		t.Fatal(page, e)
	}

	otherID := makeExpense(t, s, a, "expense-document-other", expenseInput())
	if _, e = s.LinkDocument(ctx, a, otherID, "expense-document-foreign", expenses.LinkDocumentInput{Version: 1, DocumentID: documentID, DocumentVersion: 2}); e != core.Conflict {
		t.Fatal(e)
	}

	rollbackDocument := insertExpenseDocument(t, s.Core, a, "rollback.pdf")
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER expense_document_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic expense document failure'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.LinkDocument(ctx, a, id, "expense-document-rollback", expenses.LinkDocumentInput{Version: 2, DocumentID: rollbackDocument, DocumentVersion: 1}); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER expense_document_event_failure`); e != nil {
		t.Fatal(e)
	}
	var linked sql.NullString
	if e = s.Core.DB.QueryRow(`SELECT entity_type FROM documents WHERE id=?`, rollbackDocument).Scan(&linked); e != nil {
		t.Fatal(e)
	}
	var linkCount int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM expense_documents WHERE document_id=?`, rollbackDocument).Scan(&linkCount); e != nil || linked.Valid || linkCount != 0 {
		t.Fatal(linked, linkCount, e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Version != 2 {
		t.Fatal(o, e)
	}

	if _, e = s.State(ctx, a, id, "expense-document-delete", expenses.StateInput{Version: 2, Reason: "Ocultar comprobantes del gasto eliminado"}, false); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Documents(ctx, a, id, 1, 25); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.Core.DocumentMetadata(ctx, s.Core.DB, a, documentID); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.State(ctx, a, id, "expense-document-restore", expenses.StateInput{Version: 3, Reason: "Restaurar comprobantes revisados"}, true); e != nil {
		t.Fatal(e)
	}
	page, e = s.Documents(ctx, a, id, 1, 25)
	if e != nil || page.Total != 1 || page.Data[0].ID != documentID {
		t.Fatal(page, e)
	}

	var count int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM expense_documents WHERE expense_id=?`, id).Scan(&count); e != nil || count != 1 {
		t.Fatal(fmt.Sprintf("links=%d err=%v", count, e))
	}
}
