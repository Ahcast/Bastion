package integration

import (
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"io"
	"log/slog"
	"net/http"
	"testing"
)

func TestOrderTrashRestorePreservesIdentityAllowsExternalNumberReuseAndBlocksMutations(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	p := purchaseOrderInput("", "")
	v, e := s.Save(ctx, a, "", "acceptance-source", p)
	id := customerResult(t, v, e)["id"]
	source, e := s.Get(ctx, a, id)
	if e != nil || source.Calculation.Profit != "110.00" || source.Calculation.ProfitVersion != 1 {
		t.Fatal(source, e)
	}
	intent := purchaseorders.TrashInput{Version: 1, Reason: "Registro sintético duplicado"}
	v, e = s.Trash(ctx, a, id, "order-trash", intent, false)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v, e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Deleted == nil || o.Version != 2 || o.Items[0].ID != source.Items[0].ID || o.Calculation != source.Calculation {
		t.Fatal(o, e)
	}
	list, e := s.List(ctx, a, purchaseorders.Filter{Page: 1, PerPage: 25})
	if e != nil || list.Total != 0 {
		t.Fatal(list, e)
	}
	list, e = s.List(ctx, a, purchaseorders.Filter{Page: 1, PerPage: 25, Deleted: true})
	if e != nil || list.Total != 1 || list.Data[0].Deleted == nil {
		t.Fatal(list, e)
	}
	v, e = s.Trash(ctx, a, id, "order-trash", intent, false)
	if customerResult(t, v, e)["version"] != "2" {
		t.Fatal(v, e)
	}
	intent.Reason = "Otra intención"
	if _, e = s.Trash(ctx, a, id, "order-trash", intent, false); e != core.Conflict {
		t.Fatal(e)
	}
	p.Version = 2
	if _, e = s.Save(ctx, a, id, "order-trash-edit", p); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Transition(ctx, a, id, "order-trash-transition", 0, purchaseorders.TransitionInput{Version: 2, ToStatus: "confirmed"}); e != core.Conflict {
		t.Fatal(e)
	}
	meta := cloneInput()
	meta.Version = 2
	if _, e = s.Clone(ctx, a, id, "order-trash-clone", meta); e != core.Conflict {
		t.Fatal(e)
	}
	p.Version = 0
	v, e = s.Save(ctx, a, "", "order-same-external-number", p)
	otherID := customerResult(t, v, e)["id"]
	other, e := s.Get(ctx, a, otherID)
	if e != nil || otherID == id || other.OrderNumber != source.OrderNumber {
		t.Fatal(other, e)
	}
	v, e = s.Trash(ctx, a, id, "order-restore", purchaseorders.TrashInput{Version: 2, Reason: "Corrección"}, true)
	if customerResult(t, v, e)["version"] != "3" {
		t.Fatal(v, e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.ID != source.ID || o.OrderNumber != source.OrderNumber || o.Deleted != nil || o.Items[0].ID != source.Items[0].ID || o.Calculation != source.Calculation {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, id, "order-restore", purchaseorders.TrashInput{Version: 2, Reason: "Corrección"}, true); e == nil {
		t.Fatal("reauth replay bypass")
	}
}

func TestOrderTrashProtectsDocumentsOriginClonesAndOperationalEvidence(t *testing.T) {
	s, a, id, _, doc, _ := orderDocumentFixture(t)
	if _, e := s.LinkDocument(ctx, a, id, "trash-linked-doc", purchaseorders.DocumentInput{DocumentID: doc, DocumentVersion: 1, Version: 1}); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Trash(ctx, a, id, "trash-linked-order", purchaseorders.TrashInput{Version: 2, Reason: "Test"}, false); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e := s.Core.TrashFile(ctx, a, doc, "trash-source-document", 2, false); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Trash(ctx, a, id, "trash-linked-order-again", purchaseorders.TrashInput{Version: 2, Reason: "Test"}, false); e != core.Conflict {
		t.Fatal(e)
	}
	p := purchaseOrderInput("", "")
	p.OrderNumber = "OC-PLAIN"
	v, e := s.Save(ctx, a, "", "trash-plain", p)
	plain := customerResult(t, v, e)["id"]
	v, e = s.Clone(ctx, a, plain, "trash-clone-source", cloneInput())
	child := customerResult(t, v, e)["id"]
	if _, e = s.Trash(ctx, a, plain, "trash-has-clones", purchaseorders.TrashInput{Version: 1, Reason: "Test"}, false); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, child, "trash-independent-clone", purchaseorders.TrashInput{Version: 1, Reason: "Test"}, false); e != nil {
		t.Fatal(e)
	}
	p.OrderNumber = "OC-ADVANCED"
	v, e = s.Save(ctx, a, "", "trash-advanced", p)
	advanced := customerResult(t, v, e)["id"]
	if _, e = s.Transition(ctx, a, advanced, "trash-source-advance", 1, purchaseorders.TransitionInput{Version: 1, ToStatus: "purchased"}); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, advanced, "trash-advanced-denied", purchaseorders.TrashInput{Version: 2, Reason: "Test"}, false); e != core.Conflict {
		t.Fatal(e)
	}
}

func TestOrderTrashRollbackScopeAndRevocation(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	v, e := s.Save(ctx, a, "", "trash-rollback-source", purchaseOrderInput("", ""))
	id := customerResult(t, v, e)["id"]
	p := purchaseorders.TrashInput{Version: 1, Reason: "Test"}
	other := customerActor(t, s.Core, "self")
	grantPurchaseOrders(t, s.Core, other, "self")
	if _, e = s.Trash(ctx, other, id, "trash-foreign", p, false); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER test_order_trash_event BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, id, "trash-rollback", p, false); e == nil {
		t.Fatal("event failure ignored")
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Deleted != nil || o.Version != 1 {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER test_order_trash_event`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, id, "trash-rollback", p, false); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN roles r ON r.id=rp.role_id WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.delete'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Trash(ctx, a, id, "trash-rollback", p, false); e != core.Forbidden {
		t.Fatal(e)
	}
	if _, e = s.Get(ctx, a, id); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.List(ctx, a, purchaseorders.Filter{Page: 1, PerPage: 25, Deleted: true}); e != core.Forbidden {
		t.Fatal(e)
	}
}

func TestOrderProfitMigrationBackfillsStoredDecimalValues(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	p := purchaseOrderInput("", "")
	p.AdditionalExpenses = "200"
	v, e := s.Save(ctx, a, "", "profit-backfill-source", p)
	id := customerResult(t, v, e)["id"]
	before, e := s.Get(ctx, a, id)
	if e != nil || before.Calculation.Profit != "-80.01" {
		t.Fatal(before, e)
	}
	if _, e = s.Core.DB.Exec(`ALTER TABLE customer_purchase_orders DROP INDEX customer_po_deleted,DROP COLUMN deleted_at,DROP COLUMN profit,DROP COLUMN profit_version`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM schema_migrations WHERE name='0020_customer_purchase_order_acceptance.sql'`); e != nil {
		t.Fatal(e)
	}
	if e = database.Migrate(ctx, s.Core.DB); e != nil {
		t.Fatal(e)
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Calculation != before.Calculation || after.Version != before.Version || after.Items[0].ID != before.Items[0].ID {
		t.Fatal(after, e)
	}
	if e = database.Ready(ctx, s.Core.DB); e != nil {
		t.Fatal(e)
	}
}

func TestOrderTrashHTTPRequiresReasonCSRFAndReauth(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	v, e := s.Save(ctx, a, "", "trash-http-source", purchaseOrderInput("", ""))
	id := customerResult(t, v, e)["id"]
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e = r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	path := "/api/v1/purchase-orders/" + id
	for _, body := range []any{map[string]any{"version": "1"}, map[string]any{"version": 1, "reason": "Test"}, map[string]any{"version": "1", "reason": ""}, map[string]any{"version": "1", "reason": "Test", "extra": true}} {
		status, out := purchaseOrderHTTP(t, h, "DELETE", path, body, cookie, csrf, s.Core.Config.Origin, "trash-http-invalid")
		if status != 422 {
			t.Fatal(status, out)
		}
	}
	body := map[string]any{"version": "1", "reason": "Test"}
	status, out := purchaseOrderHTTP(t, h, "DELETE", path, body, cookie, "", s.Core.Config.Origin, "trash-http-no-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "DELETE", path, body, cookie, csrf, s.Core.Config.Origin, "trash-http-valid")
	if status != 403 {
		t.Fatal("reauth", status, out)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, security.Hash(token)); e != nil {
		t.Fatal(e)
	}
	status, out = purchaseOrderHTTP(t, h, "DELETE", path, body, cookie, csrf, s.Core.Config.Origin, "trash-http-valid")
	if status != 200 || out["deleted_at"] == nil {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "GET", "/api/v1/purchase-orders?deleted=invalid", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 422 {
		t.Fatal(status, out)
	}
}

func TestOrderTrashCannotHideConvertedQuoteAndSharesProfit(t *testing.T) {
	q, s, a := conversionFixture(t)
	quote := acceptedConversionQuote(t, q, a, "trash-converted-quote", quoteInput())
	before, e := q.Get(ctx, a, quote)
	if e != nil {
		t.Fatal(e)
	}
	v, e := q.ConvertToPurchaseOrder(ctx, a, quote, "trash-conversion", conversionInput())
	id := customerResult(t, v, e)["purchase_order_id"]
	if _, e = s.Trash(ctx, a, id, "trash-converted-order", purchaseorders.TrashInput{Version: 1, Reason: "Test"}, false); e != core.Conflict {
		t.Fatal(e)
	}
	o, e := s.Get(ctx, a, id)
	if e != nil || o.Deleted != nil || o.Version != 1 {
		t.Fatal(o, e)
	}
	margin, e := finance.Profit(o.Calculation.Base15, o.Calculation.Base18, o.Calculation.Exempt, o.Calculation.Exonerated, o.Calculation.ProductsInvestment)
	if e != nil || margin != before.Calculation.Margin {
		t.Fatal(margin, e, before.Calculation)
	}
}
