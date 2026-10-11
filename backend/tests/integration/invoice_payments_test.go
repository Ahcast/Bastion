package integration

import (
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
)

func invoicePaymentFixture(t *testing.T) (*invoices.Service, core.Actor, string, invoices.PaymentInput) {
	t.Helper()
	s, a, id, p := invoiceEmissionFixture(t)
	if _, e := s.Emit(ctx, a, id, "payment-emit", p); e != nil {
		t.Fatal(e)
	}
	return s, a, id, invoices.PaymentInput{Version: 2, Date: invoiceToday(), Method: "Transferencia", Currency: "HNL", Amount: "50.00", Reference: "REFERENCIA-SINTETICA", Reason: "Cobro sintético"}
}
func paymentResult(t *testing.T, v any, e error) invoices.PaymentResult {
	t.Helper()
	if e != nil {
		t.Fatal(e)
	}
	b, e := json.Marshal(v)
	if e != nil {
		t.Fatal(e)
	}
	var r invoices.PaymentResult
	if e = json.Unmarshal(b, &r); e != nil {
		t.Fatal(e)
	}
	return r
}
func TestInvoicePaymentsRoundTripVoidReplayAndFrozenEvidence(t *testing.T) {
	s, a, id, p := invoicePaymentFixture(t)
	before, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	v, e := s.RegisterPayment(ctx, a, id, "payment-first", p)
	first := paymentResult(t, v, e)
	if first.Version != 3 || first.PaidTotal != "50.00" || first.Balance != "151.25" || first.CommercialStatus != "partially_paid" {
		t.Fatal(first)
	}
	v, e = s.RegisterPayment(ctx, a, id, "payment-first", p)
	replay := paymentResult(t, v, e)
	if replay != first {
		t.Fatal(replay, first)
	}
	p.Version = 3
	p.Amount = "151.25"
	p.Method = "Siafi"
	v, e = s.RegisterPayment(ctx, a, id, "payment-full", p)
	full := paymentResult(t, v, e)
	if full.Balance != "0.00" || full.CommercialStatus != "paid" {
		t.Fatal(full)
	}
	ledger, e := s.Payments(ctx, a, id, 1, 1)
	if e != nil || ledger.Total != 2 || len(ledger.Data) != 1 || ledger.Data[0].ReceiptNumber != nil {
		t.Fatal(ledger, e)
	}
	void := invoices.VoidPaymentInput{Version: 4, PaymentVersion: 1, Reason: "Anulación sintética"}
	v, e = s.VoidPayment(ctx, a, id, first.PaymentID, "payment-void", void)
	cancel := paymentResult(t, v, e)
	if cancel.Version != 5 || cancel.PaymentVersion != 2 || cancel.PaidTotal != "151.25" || cancel.Balance != "50.00" || cancel.CommercialStatus != "partially_paid" {
		t.Fatal(cancel)
	}
	v, e = s.VoidPayment(ctx, a, id, first.PaymentID, "payment-void", void)
	if paymentResult(t, v, e) != cancel {
		t.Fatal(v)
	}
	if _, e = s.VoidPayment(ctx, a, id, first.PaymentID, "payment-void-again", invoices.VoidPaymentInput{Version: 5, PaymentVersion: 2, Reason: "Otra"}); e != core.Conflict {
		t.Fatal(e)
	}
	ledger, e = s.Payments(ctx, a, id, 1, 25)
	if e != nil || len(ledger.Data) != 2 || ledger.Version != 5 {
		t.Fatal(ledger, e)
	}
	var voided bool
	for _, row := range ledger.Data {
		if row.ID == first.PaymentID {
			voided = row.VoidedAt != nil && row.VoidedBy != nil && row.VoidReason != nil && row.Version == 2 && row.Amount == "50.00" && row.Reference == "REFERENCIA-SINTETICA"
		}
	}
	if !voided {
		t.Fatal(ledger)
	}
	after, e := s.Get(ctx, a, id)
	if e != nil || after.Calculation != before.Calculation || after.Number == nil || *after.Number != *before.Number || string(after.SAR) != string(before.SAR) || string(after.Company) != string(before.Company) || *after.CommercialStatus != "partially_paid" {
		t.Fatal(after, e)
	}
	var next uint64
	if e = s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&next); e != nil || next != 2 {
		t.Fatal(next, e)
	}
	for _, m := range []string{"Cheque", "Efectivo"} {
		p.Version = after.Version
		p.Amount = "25.00"
		p.Method = m
		v, e = s.RegisterPayment(ctx, a, id, "method-"+m, p)
		r := paymentResult(t, v, e)
		after.Version = r.Version
	}
	after, e = s.Get(ctx, a, id)
	if e != nil || after.Balance != "0.00" || *after.CommercialStatus != "paid" {
		t.Fatal(after, e)
	}
	page, e := s.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25, CommercialStatus: "paid"})
	if e != nil || page.Total != 1 || page.Data[0].CommercialStatus == nil || *page.Data[0].CommercialStatus != "paid" {
		t.Fatal(page, e)
	}
}
func TestInvoicePaymentsRejectInvalidAndIncoherentInputs(t *testing.T) {
	s, a, id, p := invoicePaymentFixture(t)
	for _, change := range []func(*invoices.PaymentInput){func(q *invoices.PaymentInput) { q.Amount = "0" }, func(q *invoices.PaymentInput) { q.Amount = "-1" }, func(q *invoices.PaymentInput) { q.Amount = "0.001" }, func(q *invoices.PaymentInput) { q.Method = "transferencia" }, func(q *invoices.PaymentInput) { q.Date = "2026-02-30" }, func(q *invoices.PaymentInput) { q.Date = "9999-12-31" }, func(q *invoices.PaymentInput) { q.Date = "1900-01-01" }, func(q *invoices.PaymentInput) { q.Reason = "" }} {
		q := p
		change(&q)
		if _, e := s.RegisterPayment(ctx, a, id, "invalid", q); e != core.Invalid {
			t.Fatal(q, e)
		}
	}
	q := p
	q.Amount = "201.26"
	if _, e := s.RegisterPayment(ctx, a, id, "over", q); e != core.Conflict {
		t.Fatal(e)
	}
	q = p
	q.Currency = "USD"
	if _, e := s.RegisterPayment(ctx, a, id, "currency", q); e != core.Conflict {
		t.Fatal(e)
	}
	q = p
	q.Version = 1
	if _, e := s.RegisterPayment(ctx, a, id, "version", q); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e := s.RegisterPayment(ctx, core.Actor{Type: "service", ServiceID: "synthetic"}, id, "technical", p); e != core.Forbidden {
		t.Fatal(e)
	}
	if _, e := s.VoidPayment(ctx, a, id, "999999", "foreign", invoices.VoidPaymentInput{Version: 2, PaymentVersion: 1, Reason: "Ajeno"}); e != core.Missing {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE invoices SET paid_total=1,balance=total-1 WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e := s.RegisterPayment(ctx, a, id, "incoherent", p); e != core.Conflict {
		t.Fatal(e)
	}
	var rows int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM payments WHERE invoice_id=?`, id).Scan(&rows); e != nil || rows != 0 {
		t.Fatal(rows, e)
	}
}
func TestInvoicePaymentsConcurrentReplayAndRollback(t *testing.T) {
	s, a, id, p := invoicePaymentFixture(t)
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER invoice_payment_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.RegisterPayment(ctx, a, id, "rollback", p); e == nil {
		t.Fatal("expected rollback")
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER invoice_payment_event_failure`); e != nil {
		t.Fatal(e)
	}
	page, e := s.Payments(ctx, a, id, 1, 25)
	if e != nil || page.Total != 0 || page.Version != 2 || page.PaidTotal != "0.00" {
		t.Fatal(page, e)
	}
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); _, e := s.RegisterPayment(ctx, a, id, "replay-twelve", p); errs <- e }()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	page, e = s.Payments(ctx, a, id, 1, 25)
	if e != nil || page.Total != 1 || page.Version != 3 || page.PaidTotal != "50.00" {
		t.Fatal(page, e)
	}
	p.Version = 3
	p.Amount = "151.25"
	other := customerActor(t, s.Core, "all")
	grantInvoice(t, s.Core, other, "all")
	errs = make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			actor := a
			if i%2 == 1 {
				actor = other
			}
			_, e := s.RegisterPayment(ctx, actor, id, fmt.Sprint("compete-", i), p)
			errs <- e
		}(i)
	}
	wg.Wait()
	close(errs)
	success, conflicts := 0, 0
	for e := range errs {
		if e == nil {
			success++
		} else if e == core.Conflict {
			conflicts++
		} else {
			t.Fatal(e)
		}
	}
	if success != 1 || conflicts != 7 {
		t.Fatal(success, conflicts)
	}
	page, e = s.Payments(ctx, a, id, 1, 25)
	if e != nil || page.Total != 2 || page.Balance != "0.00" {
		t.Fatal(page, e)
	}
	void := invoices.VoidPaymentInput{Version: 4, PaymentVersion: 1, Reason: "Void rollback"}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER invoice_void_event_failure BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.VoidPayment(ctx, a, id, page.Data[0].ID, "void-rollback", void); e == nil {
		t.Fatal("expected void rollback")
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER invoice_void_event_failure`); e != nil {
		t.Fatal(e)
	}
	page, e = s.Payments(ctx, a, id, 1, 25)
	if e != nil || page.Version != 4 || page.Data[0].VoidedAt != nil || page.Balance != "0.00" {
		t.Fatal(page, e)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='facturas.payment'`); e != nil {
		t.Fatal(e)
	}
	p.Version = 2
	p.Amount = "50.00"
	if _, e := s.RegisterPayment(ctx, a, id, "replay-twelve", p); e != core.Forbidden {
		t.Fatal(e)
	}
}

func TestInvoicePaymentHTTPStrictScopeReauthAndRevocation(t *testing.T) {
	s, a, id, p := invoicePaymentFixture(t)
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
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	raw, _ := json.Marshal(p)
	var body map[string]any
	_ = json.Unmarshal(raw, &body)
	path := "/api/v1/invoices/" + id + "/payments"
	for name := range body {
		clone := map[string]any{}
		for k, v := range body {
			clone[k] = v
		}
		delete(clone, name)
		status, out := purchaseOrderHTTP(t, h, "POST", path, clone, cookie, csrf, s.Core.Config.Origin, "missing-"+name)
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["balance"] = "0.00"
	status, out := purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, s.Core.Config.Origin, "forged")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "balance")
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, "", s.Core.Config.Origin, "csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, s.Core.Config.Origin, "reauth")
	if status != 403 || out["code"] != "reauth_required" {
		t.Fatal(status, out)
	}
	other := customerActor(t, s.Core, "self")
	grantInvoice(t, s.Core, other, "self")
	if _, e = s.Payments(ctx, other, id, 1, 25); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.RegisterPayment(ctx, other, id, "scope", p); e != core.Missing {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE user_id=?`, a.UserID); e != nil {
		t.Fatal(e)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path, body, cookie, csrf, s.Core.Config.Origin, "http-payment")
	if status != 200 || out["paid_total"] != "50.00" || out["version"] != "3" {
		t.Fatal(status, out)
	}
	pid := out["payment_id"].(string)
	voidBody := map[string]any{"version": "3", "payment_version": "1", "reason": "Void HTTP"}
	status, out = purchaseOrderHTTP(t, h, "POST", path+"/"+pid+"/void", voidBody, cookie, csrf, s.Core.Config.Origin, "http-void")
	if status != 200 || out["paid_total"] != "0.00" {
		t.Fatal(status, out)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='facturas.payment.void'`); e != nil {
		t.Fatal(e)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", path+"/"+pid+"/void", voidBody, cookie, csrf, s.Core.Config.Origin, "http-void")
	if status != 403 {
		t.Fatal(status, out)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='facturas.view'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Payments(ctx, a, id, 1, 25); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestInvoicePaymentsZeroAndLargeExactAmounts(t *testing.T) {
	for _, large := range []bool{false, true} {
		t.Run(fmt.Sprint(large), func(t *testing.T) {
			s, a, id, emit := invoiceEmissionFixture(t)
			o, e := s.Get(ctx, a, id)
			if e != nil {
				t.Fatal(e)
			}
			p := editInvoice(o)
			p.Discount = "0"
			p.AdditionalExpenses = "0"
			p.Items[0].Discount = "0"
			p.Items[0].Cost = "0"
			p.Items[0].Tax = "exempt"
			p.Items[0].Quantity = "1"
			p.Items[0].Price = "0"
			if large {
				p.Items[0].Quantity = "10000"
				p.Items[0].Price = "900719925475"
			}
			if _, e = s.Save(ctx, a, id, "exact-draft", p); e != nil {
				t.Fatal(e)
			}
			emit.Version = 2
			if _, e = s.Emit(ctx, a, id, "exact-emit", emit); e != nil {
				t.Fatal(e)
			}
			o, e = s.Get(ctx, a, id)
			if e != nil {
				t.Fatal(e)
			}
			if !large {
				if o.Calculation.Total != "0.00" || *o.CommercialStatus != "paid" {
					t.Fatal(o)
				}
				_, e = s.RegisterPayment(ctx, a, id, "zero-pay", invoices.PaymentInput{Version: 3, Date: invoiceToday(), Method: "Efectivo", Currency: "HNL", Amount: "0.01", Reason: "No saldo"})
				if e != core.Conflict {
					t.Fatal(e)
				}
				ledger, e := s.Payments(ctx, a, id, 1, 25)
				if e != nil || ledger.Total != 0 {
					t.Fatal(ledger, e)
				}
				return
			}
			if o.Calculation.Total != "9007199254750000.00" {
				t.Fatal(o.Calculation.Total)
			}
			v, e := s.RegisterPayment(ctx, a, id, "large-pay", invoices.PaymentInput{Version: 3, Date: invoiceToday(), Method: "Cheque", Currency: "HNL", Amount: "9007199254749999.99", Reason: "Cobro exacto grande"})
			result := paymentResult(t, v, e)
			if result.Balance != "0.01" || result.PaidTotal != "9007199254749999.99" {
				t.Fatal(result)
			}
		})
	}
}

func TestInvoicePaymentsTeamScopeAndReauthBeforeReplay(t *testing.T) {
	s, a, id, p := invoicePaymentFixture(t)
	other := customerActor(t, s.Core, "self")
	grantInvoice(t, s.Core, other, "team")
	r, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo pagos sintético')`)
	if e != nil {
		t.Fatal(e)
	}
	team, e := r.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE invoices SET team_id=? WHERE id=?`, team, id); e != nil {
		t.Fatal(e)
	}
	if _, e = s.RegisterPayment(ctx, other, id, "team-missing", p); e != core.Forbidden {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members(team_id,user_id) VALUES(?,?)`, team, other.UserID); e != nil {
		t.Fatal(e)
	}
	v, e := s.RegisterPayment(ctx, other, id, "team-payment", p)
	result := paymentResult(t, v, e)
	if result.PaidTotal != "50.00" {
		t.Fatal(result)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, other.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.RegisterPayment(ctx, other, id, "team-payment", p); e == nil {
		t.Fatal("replay requires reauth")
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, other.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE team_id=? AND user_id=?`, team, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.RegisterPayment(ctx, other, id, "team-payment", p); e != core.Forbidden {
		t.Fatal(e)
	}
	// The invoice owner still reads the same authoritative ledger.
	ledger, e := s.Payments(ctx, a, id, 1, 25)
	if e != nil || ledger.PaidTotal != "50.00" {
		t.Fatal(ledger, e)
	}
}
