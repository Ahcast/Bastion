package integration

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/expenses"
	"testing"
)

func TestExpenseLinkWritesAndReplaysRequireCurrentViewPermission(t *testing.T) {
	s, _ := expenseFixture(t)
	a := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, a, "all")
	grantExpenseDocuments(t, s.Core, a, "all")
	s.Core.DocumentPolicies["expenses"] = s.DocumentPolicy
	id := makeExpense(t, s, a, "expense-view-required-source", expenseInput())
	doc := insertExpenseDocument(t, s.Core, a, "expense-view-required.pdf")
	input := expenses.LinkDocumentInput{Version: 1, DocumentID: doc, DocumentVersion: 1}
	if _, e := s.LinkDocument(ctx, a, id, "expense-view-required-link", input); e != nil {
		t.Fatal(e)
	}
	refs := expenses.SetReferencesInput{Version: 2, Reason: "Lista explícita vacía", References: []expenses.ReferenceInput{}}
	if _, e := s.SetReferences(ctx, a, id, "expense-view-required-refs", refs); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='gastos.view'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.SetReferences(ctx, a, id, "expense-view-required-refs", refs); e != core.Forbidden {
		t.Fatal("reference replay without view", e)
	}
	if _, e := s.SetReferences(ctx, a, id, "expense-view-required-new", expenses.SetReferencesInput{Version: 3, Reason: "Sin view", References: []expenses.ReferenceInput{}}); e != core.Forbidden {
		t.Fatal("reference write without view", e)
	}
	if _, e := s.LinkDocument(ctx, a, id, "expense-view-required-link", input); e != core.Forbidden {
		t.Fatal("document replay without view", e)
	}
	if e := s.DocumentPolicy(ctx, s.Core.DB, a, id, true); e != core.Forbidden {
		t.Fatal("write policy without view", e)
	}
	if _, e := s.Core.TrashFile(ctx, a, doc, "expense-view-required-doc-trash", 2, false); e != core.Missing {
		t.Fatal("document trash without expense view", e)
	}
	var version uint64
	if e := s.Core.DB.QueryRow(`SELECT version FROM expenses WHERE id=?`, id).Scan(&version); e != nil || version != 3 {
		t.Fatal(version, e)
	}
}

func TestExpenseDocumentMetadataPaginationScopesAndTrashRestore(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantExpenseDocuments(t, s.Core, a, "all")
	s.Core.DocumentPolicies["expenses"] = s.DocumentPolicy
	other := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, other, "all")
	grantExpenseDocuments(t, s.Core, other, "self")
	id := makeExpense(t, s, a, "expense-metadata-source", expenseInput())
	own := insertExpenseDocument(t, s.Core, a, "private-other-owner.pdf")
	visible := insertExpenseDocument(t, s.Core, other, "visible-scope.pdf")
	if _, e := s.LinkDocument(ctx, a, id, "expense-metadata-own", expenses.LinkDocumentInput{Version: 1, DocumentID: own, DocumentVersion: 1}); e != nil {
		t.Fatal(e)
	}
	if _, e := s.LinkDocument(ctx, a, id, "expense-metadata-visible", expenses.LinkDocumentInput{Version: 2, DocumentID: visible, DocumentVersion: 1}); e != nil {
		t.Fatal(e)
	}
	page, e := s.Documents(ctx, other, id, 1, 1)
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != visible {
		t.Fatal(page, e)
	}
	page, e = s.Documents(ctx, other, id, 2, 1)
	if e != nil || page.Total != 1 || len(page.Data) != 0 {
		t.Fatal(page, e)
	}
	if _, e = s.Core.TrashFile(ctx, other, visible, "expense-metadata-trash", 2, false); e != nil {
		t.Fatal(e)
	}
	page, e = s.Documents(ctx, other, id, 1, 25)
	if e != nil || page.Total != 0 {
		t.Fatal(page, e)
	}
	page, e = s.Documents(ctx, other, id, 1, 25, true)
	if e != nil || page.Total != 1 || page.Data[0].Version != 3 || page.Data[0].ID != visible {
		t.Fatal(page, e)
	}
	if _, e = s.Core.TrashFile(ctx, other, visible, "expense-metadata-restore", 3, true); e != nil {
		t.Fatal(e)
	}
	page, e = s.Documents(ctx, other, id, 1, 25)
	if e != nil || page.Total != 1 || page.Data[0].Version != 4 {
		t.Fatal(page, e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Version != 3 || o.Calculation.Total != "135.00" {
		t.Fatal("metadata actions changed expense", o, e)
	}
}

func TestExpenseDocumentTrashVersusExpenseDeletionTwoActors(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantExpenseDocuments(t, s.Core, a, "all")
	s.Core.DocumentPolicies["expenses"] = s.DocumentPolicy
	other := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, other, "all")
	grantExpenseDocuments(t, s.Core, other, "all")
	id := makeExpense(t, s, a, "expense-race-source", expenseInput())
	doc := insertExpenseDocument(t, s.Core, a, "concurrent-receipt.pdf")
	if _, e := s.LinkDocument(ctx, a, id, "expense-race-link", expenses.LinkDocumentInput{Version: 1, DocumentID: doc, DocumentVersion: 1}); e != nil {
		t.Fatal(e)
	}
	start := make(chan struct{})
	done := make(chan error, 2)
	go func() {
		<-start
		_, e := s.State(ctx, a, id, "expense-race-domain-trash", expenses.StateInput{Version: 2, Reason: "Papelera concurrente"}, false)
		done <- e
	}()
	go func() {
		<-start
		_, e := s.Core.TrashFile(ctx, other, doc, "expense-race-receipt-trash", 2, false)
		done <- e
	}()
	close(start)
	for i := 0; i < 2; i++ {
		e := <-done
		if e != nil && e != core.Missing {
			t.Fatal("lock-order failure", e)
		}
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.DeletedAt == nil || o.Version != 3 {
		t.Fatal(o, e)
	}
	if _, e = s.Documents(ctx, other, id, 1, 25, true); e != core.Missing {
		t.Fatal("deleted expense documents visible", e)
	}
}

func TestExpenseDocumentBindingUsesCanonicalAggregateIdentity(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantExpenseDocuments(t, s.Core, a, "all")
	s.Core.DocumentPolicies["expenses"] = s.DocumentPolicy
	id := makeExpense(t, s, a, "expense-canonical-source", expenseInput())
	doc := insertExpenseDocument(t, s.Core, a, "canonical-receipt.pdf")
	input := expenses.LinkDocumentInput{Version: 1, DocumentID: doc, DocumentVersion: 1}
	v, e := s.LinkDocument(ctx, a, "0"+id, "expense-canonical-link", input)
	result := customerResult(t, v, e)
	if result["id"] != id {
		t.Fatal(result)
	}
	var bound string
	if e = s.Core.DB.QueryRow(`SELECT entity_id FROM documents WHERE id=?`, doc).Scan(&bound); e != nil || bound != id {
		t.Fatal(bound, e)
	}
	for _, readID := range []string{id, "0" + id} {
		page, e := s.Documents(ctx, a, readID, 1, 25)
		if e != nil || page.Total != 1 || page.Data[0].ID != doc {
			t.Fatal(readID, page, e)
		}
	}
}
