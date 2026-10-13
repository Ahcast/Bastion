package integration

import (
	"encoding/json"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/collaboration"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
)

func TestCollaborationRealOwnerLinksCommentsTasksAndRevocation(t *testing.T) {
	ps, machine := processFixture(t)
	s := ps.Core
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	grantCustomer(t, s, a, "all")
	grantQuote(t, s, a, "all")
	grantInvoice(t, s, a, "all")
	grantPurchaseOrders(t, s, a, "all")
	batch := processes.Batch{ID: "collaboration-owner-process", Schema: "1.0", Processes: []processes.Observation{obs(time.Now().UTC().Add(-time.Minute), "Entidad sintética")}}
	if _, e := ps.Import(ctx, machine, "HC-synthetic-test", batch.ID, batch); e != nil {
		t.Fatal(e)
	}
	var pid string
	if e := s.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM processes LIMIT 1`).Scan(&pid); e != nil {
		t.Fatal(e)
	}
	cs := &customers.Service{Core: s}
	qs := &quotes.Service{Core: s, Customers: cs, Processes: ps}
	pos := &purchaseorders.Service{Core: s, Customers: cs, Suppliers: &suppliers.Service{Core: s}}
	is := &invoices.Service{Core: s, Customers: cs}
	qid := makeQuote(t, qs, a, "collaboration-owner-quote", quoteInput())
	pv, pe := pos.Save(ctx, a, "", "collaboration-owner-order", purchaseOrderInput("", ""))
	poid := customerResult(t, pv, pe)["id"]
	iv, ie := is.Save(ctx, a, "", "collaboration-owner-invoice", invoiceInput())
	iid := customerResult(t, iv, ie)["id"]
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)
	service.ResourcePolicies["processes"] = collaboration.OwnerResourcePolicy("procesos.view", ps)
	service.ResourcePolicies["quotes"] = collaboration.OwnerResourcePolicy("cotizaciones.view", qs)
	service.ResourcePolicies["purchase_orders"] = collaboration.OwnerResourcePolicy("ordenes.view", pos)
	service.ResourcePolicies["invoices"] = collaboration.OwnerResourcePolicy("facturas.view", is)
	entryID := createConversationEntry(t, s, a, service, "collaboration-real-owners")
	beforeQ, e := qs.Get(ctx, a, qid)
	if e != nil {
		t.Fatal(e)
	}
	beforePO, e := pos.Get(ctx, a, poid)
	if e != nil {
		t.Fatal(e)
	}
	beforeI, e := is.Get(ctx, a, iid)
	if e != nil {
		t.Fatal(e)
	}
	before, _ := json.Marshal([]any{beforeQ, beforePO, beforeI})
	links := []collaboration.LinkInput{{ResourceType: "processes", ResourceID: pid, ResourceVersion: 1}, {ResourceType: "quotes", ResourceID: qid, ResourceVersion: 1}, {ResourceType: "purchase_orders", ResourceID: poid, ResourceVersion: 1}, {ResourceType: "invoices", ResourceID: iid, ResourceVersion: 1}}
	if _, e = service.SetLinks(ctx, a, entryID, "real-owner-links", collaboration.SetLinksInput{Version: 1, Links: links}); e != nil {
		t.Fatal(e)
	}
	visible, e := service.Links(ctx, a, entryID)
	if e != nil || len(visible) != 4 {
		t.Fatal("four real owner links", visible, e)
	}
	for _, link := range links {
		if _, e = service.CreateComment(ctx, a, link.ResourceType, link.ResourceID, "real-comment-"+link.ResourceType, collaboration.CommentInput{Body: "Nota de seguimiento"}); e != nil {
			t.Fatal(link.ResourceType, e)
		}
		comments, err := service.Comments(ctx, a, link.ResourceType, link.ResourceID, 1, 25)
		if err != nil || comments.Total != 1 {
			t.Fatal("real resource conversation", link.ResourceType, comments, err)
		}
	}
	reviewer := customerActor(t, s, "self")
	grantCollaboration(t, s, reviewer, "all")
	grantQuote(t, s, reviewer, "all")
	if _, e = service.CreateTask(ctx, a, "real-owner-task", collaboration.TaskInput{Title: "Revisar cotización", AssigneeID: reviewer.UserID, ResourceType: "quotes", ResourceID: qid}); e != nil {
		t.Fatal(e)
	}
	tasks, e := service.Tasks(ctx, reviewer, collaboration.TaskFilter{View: "mine", Page: 1, PerPage: 25})
	if e != nil || tasks.Total != 1 {
		t.Fatal("linked task visible", tasks, e)
	}
	afterQ, e := qs.Get(ctx, a, qid)
	if e != nil {
		t.Fatal(e)
	}
	afterPO, e := pos.Get(ctx, a, poid)
	if e != nil {
		t.Fatal(e)
	}
	afterI, e := is.Get(ctx, a, iid)
	if e != nil {
		t.Fatal(e)
	}
	after, _ := json.Marshal([]any{afterQ, afterPO, afterI})
	if string(before) != string(after) {
		t.Fatal("Collaboration mutated real commercial aggregates")
	}
	if _, e = s.DB.Exec(`DELETE ur FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=? AND r.name='quotes-all'`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	tasks, e = service.Tasks(ctx, reviewer, collaboration.TaskFilter{View: "mine", Page: 1, PerPage: 25})
	if e != nil || tasks.Total != 0 {
		t.Fatal("revoked owner task leaked", tasks, e)
	}
	if _, e = service.Comments(ctx, reviewer, "quotes", qid, 1, 25); e != core.Missing {
		t.Fatal("revoked real owner conversation", e)
	}
	edited := editQuote(afterQ)
	edited.Terms = "Términos revisados por el propietario"
	if _, e = qs.Save(ctx, a, qid, "real-owner-quote-edit", edited); e != nil {
		t.Fatal(e)
	}
	visible, e = service.Links(ctx, a, entryID)
	if e != nil {
		t.Fatal(e)
	}
	stale := false
	for _, link := range visible {
		if link.ResourceType == "quotes" {
			stale = link.Stale && link.ResourceVersion == 1 && link.CurrentVersion == 2
		}
	}
	if !stale {
		t.Fatal("real owner version snapshot was rewritten", visible)
	}
	if _, e = qs.Trash(ctx, a, qid, "real-owner-quote-trash", 2, false); e != nil {
		t.Fatal(e)
	}
	if _, e = pos.Trash(ctx, a, poid, "real-owner-order-trash", purchaseorders.TrashInput{Version: 1, Reason: "Prueba de acceso propietario"}, false); e != nil {
		t.Fatal(e)
	}
	visible, e = service.Links(ctx, a, entryID)
	if e != nil || len(visible) != 2 {
		t.Fatal("deleted owners remain visible", visible, e)
	}
	if _, e = service.SetLinks(ctx, a, entryID, "real-owner-links", collaboration.SetLinksInput{Version: 1, Links: links}); e != core.Missing {
		t.Fatal("deleted owner replay bypass", e)
	}
	var evidence int
	if e = s.DB.QueryRow(`SELECT COUNT(*) FROM collaboration_links WHERE entry_id=?`, entryID).Scan(&evidence); e != nil || evidence != 4 {
		t.Fatal("hidden links destroyed evidence", evidence, e)
	}
	delete(service.ResourcePolicies, "processes")
	visible, e = service.Links(ctx, a, entryID)
	if e != nil || len(visible) != 1 || visible[0].ResourceType != "invoices" {
		t.Fatal("unregistered owner disclosed", visible, e)
	}
}

func TestCollaborationCancelledRealInvoiceHidesLinksAndConversation(t *testing.T) {
	is, a, id, emission := invoiceEmissionFixture(t)
	if _, e := is.Emit(ctx, a, id, "collaboration-real-emit", emission); e != nil {
		t.Fatal(e)
	}
	issued, e := is.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	s := is.Core
	grantCollaboration(t, s, a, "all")
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)
	service.ResourcePolicies["invoices"] = collaboration.OwnerResourcePolicy("facturas.view", is)
	entry := createConversationEntry(t, s, a, service, "collaboration-cancelled-invoice")
	input := collaboration.SetLinksInput{Version: 1, Links: []collaboration.LinkInput{{ResourceType: "invoices", ResourceID: id, ResourceVersion: issued.Version}}}
	if _, e = service.SetLinks(ctx, a, entry, "invoice-live-link", input); e != nil {
		t.Fatal(e)
	}
	if _, e = service.CreateComment(ctx, a, "invoices", id, "invoice-live-comment", collaboration.CommentInput{Body: "Seguimiento documental"}); e != nil {
		t.Fatal(e)
	}
	if _, e = is.Cancel(ctx, a, id, "collaboration-owner-cancel", invoices.CancelInput{Version: issued.Version, Reason: "Corrección sintética", OriginalAndCopyMarkedAndArchived: true}); e != nil {
		t.Fatal(e)
	}
	links, e := service.Links(ctx, a, entry)
	if e != nil || len(links) != 0 {
		t.Fatal("cancelled invoice link visible", links, e)
	}
	if _, e = service.Comments(ctx, a, "invoices", id, 1, 25); e != core.Missing {
		t.Fatal("cancelled invoice comments visible", e)
	}
	if _, e = service.SetLinks(ctx, a, entry, "invoice-live-link", input); e != core.Missing {
		t.Fatal("cancelled invoice replay", e)
	}
}

func TestCollaborationRealWorkflowInboxUsesOwnerAccessAndDecisionState(t *testing.T) {
	qs, requester, reviewer, id := approvalFixture(t)
	s := qs.Core
	grantCollaboration(t, s, requester, "all")
	grantCollaboration(t, s, reviewer, "all")
	service := &collaboration.Service{Core: s, Approvals: qs.Approvals, ResourcePolicies: map[string]collaboration.ResourcePolicy{"quotes": collaboration.OwnerResourcePolicy("cotizaciones.view", qs)}}
	pending := requestApproval(t, qs, requester, id, "collaboration-inbox", 3)
	page, e := service.PendingApprovals(ctx, reviewer, 1, 25)
	if e != nil || page.Total != 1 || page.Data[0].ID != pending.ApprovalID {
		t.Fatal("real Workflow pending", page, e)
	}
	own, e := service.PendingApprovals(ctx, requester, 1, 25)
	if e != nil || own.Total != 0 {
		t.Fatal("self approval inbox", own, e)
	}
	var status string
	if e = s.DB.QueryRow(`SELECT status FROM approvals WHERE id=?`, pending.ApprovalID).Scan(&status); e != nil || status != "pending" {
		t.Fatal("inbox mutated Workflow", status, e)
	}
	delete(service.ResourcePolicies, "quotes")
	page, e = service.PendingApprovals(ctx, reviewer, 1, 1)
	if e != nil || page.Total != 0 {
		t.Fatal("disabled owner counted", page, e)
	}
	service.ResourcePolicies["quotes"] = collaboration.OwnerResourcePolicy("cotizaciones.view", qs)
	if _, e = s.DB.Exec(`DELETE ur FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=? AND r.name='quotes-all'`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	page, e = service.PendingApprovals(ctx, reviewer, 1, 25)
	if e != nil || page.Total != 0 {
		t.Fatal("revoked decision/view inbox", page, e)
	}
	grantQuote(t, s, reviewer, "all")
	decideApproval(t, qs, reviewer, id, pending, 3, "approved")
	page, e = service.PendingApprovals(ctx, reviewer, 1, 25)
	if e != nil || page.Total != 0 {
		t.Fatal("decided approval pending", page, e)
	}
}
