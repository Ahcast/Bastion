package integration

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
)

func orderDocumentFixture(t *testing.T) (*purchaseorders.Service, core.Actor, string, purchaseorders.Input, string, previewObjects) {
	t.Helper()
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	s.Core.DocumentPolicies["purchase_orders"] = s.DocumentPolicy
	p := purchaseOrderInput("", "")
	p.Items = append(p.Items, p.Items[0])
	p.Items[1].Description = "Segundo equipo"
	v, e := s.Save(ctx, a, "", "documents-order-create", p)
	id := customerResult(t, v, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p.Version = o.Version
	for i := range p.Items {
		p.Items[i] = o.Items[i].Item
	}
	content := "%PDF-1.7\nsynthetic"
	raw, _ := hexBytes(security.Hash(content))
	f := previewObjects{objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64Checksum(raw), Prefix: []byte(content)}}}
	v, e = s.Core.CreateFile(ctx, a, "", "order-upload-file", core.FileRequest{Filename: "order.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, f)
	if e != nil {
		t.Fatal(e)
	}
	return s, a, id, p, v.(map[string]any)["id"].(string), f
}
func TestOrderDocumentsStableItemIdentityQuarantineTrashRestore(t *testing.T) {
	s, a, id, p, doc, f := orderDocumentFixture(t)
	item := p.Items[0].ID
	v, e := s.LinkDocument(ctx, a, id, "order-document-link", purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1, ItemID: &item})
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	p.Version = 2
	p.Items[0], p.Items[1] = p.Items[1], p.Items[0]
	v, e = s.Save(ctx, a, id, "order-reorder-attached", p)
	if customerResult(t, v, e)["version"] != "3" {
		t.Fatal(v)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Items[1].ID != item {
		t.Fatal("identity lost", o, e)
	}
	var linked string
	var position int
	if e = s.Core.DB.QueryRow(`SELECT CAST(l.item_id AS CHAR),i.position FROM customer_purchase_order_documents l JOIN customer_purchase_order_items i ON i.id=l.item_id WHERE l.document_id=?`, doc).Scan(&linked, &position); e != nil || linked != item || position != 1 {
		t.Fatal(linked, position, e)
	}
	list, e := s.Documents(ctx, a, id, item, 1, 25, false)
	if e != nil || list.(map[string]any)["total"] != 1 {
		t.Fatal(list, e)
	}
	if e = s.Core.FinalizeFile(ctx, a, doc, f); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != core.Forbidden {
		t.Fatal("quarantine", e)
	}
	if _, e = s.Core.PreviewFile(ctx, a, doc, 0, f); e != core.Forbidden {
		t.Fatal("quarantine preview", e)
	}
	if e = s.Core.ScanFile(ctx, a, doc, 1, true, "synthetic-order-clean-scan"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.PreviewFile(ctx, a, doc, 0, f); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.TrashFile(ctx, a, doc, "order-document-trash", 2, false); e != nil {
		t.Fatal(e)
	}
	p.Version = 3
	p.Items = p.Items[:1]
	if _, e = s.Save(ctx, a, id, "order-delete-linked-item", p); e != core.Conflict {
		t.Fatal("trashed link lost", e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Version != 3 || len(o.Items) != 2 || o.Items[1].ID != item {
		t.Fatal("rollback", o, e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.Core.TrashFile(ctx, a, doc, "order-document-restore", 3, true); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DownloadFile(ctx, a, doc, 0, f); e != nil {
		t.Fatal(e)
	}
}
func TestOrderDocumentReplayReauthorizationConcurrent(t *testing.T) {
	s, a, id, p, doc, _ := orderDocumentFixture(t)
	item := p.Items[0].ID
	input := purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1, ItemID: &item}
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, e := s.LinkDocument(ctx, a, id, "order-link-concurrent", input)
			errs <- e
		}()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_order_documents`).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	if _, e := s.LinkDocument(ctx, a, id, "another-order-intent", input); e != core.Conflict {
		t.Fatal(e)
	}
	other := customerActor(t, s.Core, "self")
	grantPurchaseOrders(t, s.Core, other, "self")
	if _, e := s.LinkDocument(ctx, other, id, "foreign-link-key", input); e != core.Missing {
		t.Fatal("foreign OC", e)
	}
	if _, e := s.Documents(ctx, other, id, "", 1, 25, false); e != core.Missing {
		t.Fatal("foreign list", e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='ordenes.documents.write'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.LinkDocument(ctx, a, id, "order-link-concurrent", input); e == nil {
		t.Fatal("revoked replay")
	}
}
func TestOrderItemIDsAndDocumentLinkRollback(t *testing.T) {
	s, a, id, p, doc, _ := orderDocumentFixture(t)
	p.Items[1].ID = p.Items[0].ID
	if _, e := s.Save(ctx, a, id, "duplicate-item-id", p); e != core.Conflict {
		t.Fatal(e)
	}
	p.Items[1].ID = "999999999"
	if _, e := s.Save(ctx, a, id, "foreign-item-id", p); e != core.Missing {
		t.Fatal(e)
	}
	item := "999999999"
	if _, e := s.LinkDocument(ctx, a, id, "document-foreign-item", purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1, ItemID: &item}); e != core.Missing {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_order_document_event BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic document event outage'`); e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { s.Core.DB.Exec(`DROP TRIGGER IF EXISTS reject_order_document_event`) })
	if _, e := s.LinkDocument(ctx, a, id, "document-atomic-link", purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1}); e == nil {
		t.Fatal("event failure swallowed")
	}
	var count int
	var entity *string
	var version uint64
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_order_documents`).Scan(&count); e != nil || count != 0 {
		t.Fatal(count, e)
	}
	if e := s.Core.DB.QueryRow(`SELECT entity_type,version FROM documents WHERE id=?`, doc).Scan(&entity, &version); e != nil || entity != nil || version != 1 {
		t.Fatal(entity, version, e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Version != 1 {
		t.Fatal(o, e)
	}
}

func TestOrderDocumentHTTPStrictDTOAndCSRF(t *testing.T) {
	s, a, id, _, doc, _ := orderDocumentFixture(t)
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e := r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, security.Hash(token)); e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	path := "/api/v1/purchase-orders/" + id + "/documents"
	valid := map[string]any{"document_id": doc, "document_version": "1", "version": "1", "item_id": nil}
	for _, mode := range []string{"omitted_item", "unknown", "numeric_version", "empty_item", "missing_key"} {
		body := map[string]any{}
		for k, v := range valid {
			body[k] = v
		}
		key := "strict-document-" + mode
		switch mode {
		case "omitted_item":
			delete(body, "item_id")
		case "unknown":
			body["extra"] = true
		case "numeric_version":
			body["version"] = 1
		case "empty_item":
			body["item_id"] = ""
		case "missing_key":
			key = ""
		}
		status, out := purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, s.Core.Config.Origin, key)
		if status != 422 {
			t.Fatal(mode, status, out)
		}
	}
	status, out := purchaseOrderHTTP(t, h, "POST", path, valid, cookie, "", s.Core.Config.Origin, "document-no-csrf")
	if status != 403 {
		t.Fatal("CSRF", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, valid, cookie, csrf, s.Core.Config.Origin, "document-http-valid")
	if status != 200 || out["version"] != "2" {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", path+"?deleted=invalid", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal("query", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", path, nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 200 || out["total"] != float64(1) {
		t.Fatal("list", status, out)
	}
}

func TestOrderDocumentsRequireBothScopesAndTeamMembership(t *testing.T) {
	s, a, id, _, doc, f := orderDocumentFixture(t)
	if _, e := s.LinkDocument(ctx, a, id, "document-scope-header", purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1}); e != nil {
		t.Fatal(e)
	}
	other := customerActor(t, s.Core, "self")
	grantPurchaseOrders(t, s.Core, other, "all")
	if _, e := s.Core.DB.Exec(`INSERT INTO roles(name) VALUES('order-document-reader')`); e != nil {
		t.Fatal(e)
	}
	var role string
	if e := s.Core.DB.QueryRow(`SELECT id FROM roles WHERE name='order-document-reader'`).Scan(&role); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO role_permissions VALUES(?,'core.documents.read','self')`, role); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO user_roles VALUES(?,?)`, other.UserID, role); e != nil {
		t.Fatal(e)
	}
	v, e := s.Documents(ctx, other, id, "", 1, 25, false)
	if e != nil || v.(map[string]any)["total"] != 0 {
		t.Fatal("technical self scope", v, e)
	}
	if _, e := s.Core.DownloadFile(ctx, other, doc, 0, f); e != core.Missing {
		t.Fatal("technical scope", e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Document team')`); e != nil {
		t.Fatal(e)
	}
	var team string
	if e := s.Core.DB.QueryRow(`SELECT id FROM teams WHERE name='Document team'`).Scan(&team); e != nil {
		t.Fatal(e)
	}
	for _, q := range []string{`UPDATE documents SET team_id=? WHERE id=?`, `UPDATE customer_purchase_orders SET team_id=? WHERE id=?`} {
		target := doc
		if q == `UPDATE customer_purchase_orders SET team_id=? WHERE id=?` {
			target = id
		}
		if _, e := s.Core.DB.Exec(q, team, target); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.Core.DB.Exec(`UPDATE role_permissions SET scope='team' WHERE role_id=?`, role); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE role_permissions SET scope='team' WHERE role_id IN(SELECT role_id FROM user_roles WHERE user_id=?) AND permission LIKE 'ordenes.%'`, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	v, e = s.Documents(ctx, other, id, "", 1, 25, false)
	if e != nil || v.(map[string]any)["total"] != 1 {
		t.Fatal("team scope", v, e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM team_members WHERE user_id=?`, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Documents(ctx, other, id, "", 1, 25, false); e == nil {
		t.Fatal("removed team allowed")
	}
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET status='closed' WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if e = s.DocumentPolicy(ctx, s.Core.DB, a, id, true); e != core.Forbidden {
		t.Fatal("closed writes", e)
	}
	if e = s.DocumentPolicy(ctx, s.Core.DB, a, id, false); e != nil {
		t.Fatal("closed reads", e)
	}
}
