package integration

import (
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
)

func TestExpenseReferenceOwnerPortsEnforceLiveVisibilityAndTerminalState(t *testing.T) {
	// Process: owner service supplies the live snapshot under its own authorization.
	ps, _ := processFixture(t)
	processActor := admin(t, ps.Core)
	var processID string
	if e := ps.Core.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM processes ORDER BY id LIMIT 1`).Scan(&processID); e != nil {
		t.Fatal(e)
	}
	tx, e := ps.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	label, version, e := ps.ExpenseReferenceInTransaction(ctx, tx, processActor, processID)
	_ = tx.Rollback()
	if e != nil || label == "" || version == 0 {
		t.Fatal("process reference", label, version, e)
	}
	if _, e = ps.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='procesos.view'`, processActor.UserID); e != nil {
		t.Fatal(e)
	}
	tx, e = ps.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	_, _, e = ps.ExpenseReferenceInTransaction(ctx, tx, processActor, processID)
	_ = tx.Rollback()
	if e != core.Missing {
		t.Fatal("process visibility bypass", e)
	}

	// Quote: active source is readable; soft-deleted source cannot create a new reference.
	qs, qa, _ := quotesFixture(t)
	grantQuote(t, qs.Core, qa, "all")
	quoteID := makeQuote(t, qs, qa, "expense-reference-quote", quoteInput())
	tx, e = qs.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	quoteLabel, quoteVersion, e := qs.ExpenseReferenceInTransaction(ctx, tx, qa, quoteID)
	_ = tx.Rollback()
	if e != nil || quoteLabel != "COT-000001" || quoteVersion != 1 {
		t.Fatal("quote reference", quoteLabel, quoteVersion, e)
	}
	if _, e = qs.Core.DB.Exec(`UPDATE quotes SET deleted_at=UTC_TIMESTAMP(6),deleted_by=? WHERE id=?`, qa.UserID, quoteID); e != nil {
		t.Fatal(e)
	}
	tx, e = qs.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	_, _, e = qs.ExpenseReferenceInTransaction(ctx, tx, qa, quoteID)
	_ = tx.Rollback()
	if e != core.Conflict {
		t.Fatal("deleted quote accepted", e)
	}

	// Purchase order: same owner-enforced deletion rule.
	pos, _, _, poa := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, pos.Core, poa, "all")
	poValue, e := pos.Save(ctx, poa, "", "expense-reference-po", purchaseOrderInput("", ""))
	poID := customerResult(t, poValue, e)["id"]
	tx, e = pos.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	poLabel, poVersion, e := pos.ExpenseReferenceInTransaction(ctx, tx, poa, poID)
	_ = tx.Rollback()
	if e != nil || poLabel != "OC-CLIENTE-001" || poVersion != 1 {
		t.Fatal("purchase order reference", poLabel, poVersion, e)
	}
	if _, e = pos.Core.DB.Exec(`UPDATE purchase_orders SET deleted_at=UTC_TIMESTAMP(6),deleted_by=? WHERE id=?`, poa.UserID, poID); e != nil {
		t.Fatal(e)
	}
	tx, e = pos.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	_, _, e = pos.ExpenseReferenceInTransaction(ctx, tx, poa, poID)
	_ = tx.Rollback()
	if e != core.Conflict {
		t.Fatal("deleted purchase order accepted", e)
	}

	// Invoice: a cancelled source is historical evidence only, never a new live reference.
	is, ia := invoiceFixture(t)
	grantInvoice(t, is.Core, ia, "all")
	invoiceValue, e := is.Save(ctx, ia, "", "expense-reference-invoice", invoiceInput())
	invoiceID := customerResult(t, invoiceValue, e)["id"]
	tx, e = is.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	invoiceLabel, invoiceVersion, e := is.ExpenseReferenceInTransaction(ctx, tx, ia, invoiceID)
	_ = tx.Rollback()
	if e != nil || invoiceLabel != "Factura #"+invoiceID || invoiceVersion != 1 {
		t.Fatal("invoice reference", invoiceLabel, invoiceVersion, e)
	}
	if _, e = is.Core.DB.Exec(`UPDATE invoices SET status='cancelled',cancelled_at=UTC_TIMESTAMP(6),cancelled_by=?,cancel_reason='Prueba de puerto' WHERE id=?`, ia.UserID, invoiceID); e != nil {
		t.Fatal(e)
	}
	tx, e = is.Core.DB.BeginTx(ctx, nil)
	if e != nil {
		t.Fatal(e)
	}
	_, _, e = is.ExpenseReferenceInTransaction(ctx, tx, ia, invoiceID)
	_ = tx.Rollback()
	if e != core.Conflict {
		t.Fatal("cancelled invoice accepted", e)
	}

	// Keep compiler guarding the public cancellation input contract used by the owner module.
	_ = invoices.CancelInput{}
}
