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
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
)

func syntheticFiscalSettings() invoices.FiscalSettingsInput {
	return invoices.FiscalSettingsInput{Value: invoices.FiscalAuthorization{CAI: "CAI-SINTETICO-NO-VALIDO", Prefix: "TEST-", Width: 8, RangeStart: 1, RangeEnd: 100, ValidFrom: "2026-10-01", ValidUntil: "2026-10-31", Currency: "HNL", Enabled: true}, Reason: "Autorización sintética"}
}
func TestInvoiceFiscalSettingsRoundTripCounterAndPermissions(t *testing.T) {
	s, a := invoiceFixture(t)
	p := syntheticFiscalSettings()
	if _, e := s.SaveFiscalSettings(ctx, a, "fiscal-create", p); e != core.Forbidden {
		t.Fatal(e)
	}
	grantInvoice(t, s.Core, a, "all")
	if _, e := s.FiscalSettings(ctx, a); e != core.Missing {
		t.Fatal(e)
	}
	v, e := s.SaveFiscalSettings(ctx, a, "fiscal-create", p)
	result := customerResult(t, v, e)
	if result["version"] != "1" || result["next_value"] != "1" {
		t.Fatal(result)
	}
	o, e := s.FiscalSettings(ctx, a)
	if e != nil || o.Version != 1 || o.Value != p.Value || o.NextValue != 1 {
		t.Fatal(o, e)
	}
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-create", p); e != nil {
		t.Fatal(e)
	}
	p.Reason = "Alteración de intención"
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-create", p); e != core.Conflict {
		t.Fatal(e)
	}
	p.Version = 1
	p.Value.RangeStart = 2
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-jump", p); e != core.Conflict {
		t.Fatal(e)
	}
	p.Value.RangeStart = 1
	p.Value.RangeEnd = 200
	p.Value.Enabled = false
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-edit", p); e != nil {
		t.Fatal(e)
	}
	o, e = s.FiscalSettings(ctx, a)
	if e != nil || o.Version != 2 || o.NextValue != 1 || o.Value.Enabled {
		t.Fatal(o, e)
	}
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-stale", p); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-edit", p); e == nil {
		t.Fatal("replay bypassed reauth")
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='facturas.fiscal.manage'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-edit", p); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestInvoiceFiscalSettingsConcurrentReplayAndAtomicRollback(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	p := syntheticFiscalSettings()

	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_fiscal_create BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic fiscal create rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.SaveFiscalSettings(ctx, a, "fiscal-create-rollback", p); e == nil {
		t.Fatal("create event failure committed")
	}
	for _, query := range []string{`SELECT COUNT(*) FROM invoice_fiscal_settings`, `SELECT COUNT(*) FROM document_counters WHERE name='invoice'`} {
		var n int
		if e := s.Core.DB.QueryRow(query).Scan(&n); e != nil || n != 0 {
			t.Fatal(n, e)
		}
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER reject_fiscal_create`); e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); _, e := s.SaveFiscalSettings(ctx, a, "fiscal-concurrent", p); errs <- e }()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	var n int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM document_counters WHERE name='invoice'`).Scan(&n); e != nil || n != 1 {
		t.Fatal(n, e)
	}
	p.Version = 1
	p.Value.RangeEnd = 200
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_invoice_fiscal_event BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic fiscal rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.SaveFiscalSettings(ctx, a, "fiscal-rollback", p); e == nil {
		t.Fatal("event failure committed")
	}
	o, e := s.FiscalSettings(ctx, a)
	if e != nil || o.Version != 1 || o.Value.RangeEnd != 100 || o.NextValue != 1 {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER reject_invoice_fiscal_event`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-rollback", p); e != nil {
		t.Fatal(e)
	}
}
func TestInvoiceFiscalSettingsProtectsConsumedFormatAndExhaustion(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	p := syntheticFiscalSettings()
	if _, e := s.SaveFiscalSettings(ctx, a, "fiscal-create", p); e != nil {
		t.Fatal(e)
	}
	v, e := s.Save(ctx, a, "", "invoice-fiscal-evidence", invoiceInput())
	id := customerResult(t, v, e)["id"]
	if _, e = s.Core.DB.Exec(`UPDATE invoices SET invoice_number='TEST-00000001',sar_snapshot_json=JSON_OBJECT('cai','historical'),status='issued' WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE document_counters SET next_value=101 WHERE name='invoice'`); e != nil {
		t.Fatal(e)
	}
	p.Version = 1
	p.Value.Prefix = "OTHER-"
	p.Value.RangeEnd = 200
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-format", p); e != core.Conflict {
		t.Fatal(e)
	}
	p.Value.Prefix = "TEST-"
	p.Value.RangeEnd = 100
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-exhausted", p); e != core.Conflict {
		t.Fatal(e)
	}
	p.Value.Enabled = false
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-disable", p); e != nil {
		t.Fatal(e)
	}
	o, e := s.FiscalSettings(ctx, a)
	if e != nil || o.NextValue != 101 || o.Value.Enabled || o.Version != 2 {
		t.Fatal(o, e)
	}
	p.Version = 2
	p.Value.Enabled = true
	p.Value.RangeEnd = 200
	if _, e = s.SaveFiscalSettings(ctx, a, "fiscal-extend", p); e != nil {
		t.Fatal(e)
	}
	var raw string
	if e = s.Core.DB.QueryRow(`SELECT sar_snapshot_json FROM invoices WHERE id=?`, id).Scan(&raw); e != nil {
		t.Fatal(e)
	}
	var snapshot map[string]string
	if json.Unmarshal([]byte(raw), &snapshot) != nil || snapshot["cai"] != "historical" {
		t.Fatal(raw)
	}
}
func TestInvoiceFiscalSettingsHTTPStrictFieldsCSRFAndReauth(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e := r.Register(s.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, logger)

	preflight := httptest.NewRequest("OPTIONS", "/api/v1/invoices/fiscal-settings", nil)
	preflight.Header.Set("Origin", s.Core.Config.Origin)
	preflight.Header.Set("Access-Control-Request-Method", "PUT")
	rr := httptest.NewRecorder()
	h.ServeHTTP(rr, preflight)
	if rr.Code != 204 || !strings.Contains(rr.Header().Get("Access-Control-Allow-Methods"), "PUT") {
		t.Fatal(rr.Code, rr.Header())
	}
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	p := syntheticFiscalSettings()
	raw, _ := json.Marshal(p)
	var body map[string]any
	_ = json.Unmarshal(raw, &body)
	for _, name := range []string{"version", "value", "reason"} {
		clone := map[string]any{}
		for k, v := range body {
			clone[k] = v
		}
		delete(clone, name)
		status, out := purchaseOrderHTTP(t, h, "PUT", "/api/v1/invoices/fiscal-settings", clone, cookie, csrf, s.Core.Config.Origin, "fiscal-missing")
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	value := body["value"].(map[string]any)
	for name := range value {
		clone := map[string]any{}
		for k, v := range value {
			clone[k] = v
		}
		delete(clone, name)
		request := map[string]any{"version": "0", "value": clone, "reason": "Sintético"}
		status, out := purchaseOrderHTTP(t, h, "PUT", "/api/v1/invoices/fiscal-settings", request, cookie, csrf, s.Core.Config.Origin, "fiscal-value-missing")
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["next_value"] = "1"
	status, out := purchaseOrderHTTP(t, h, "PUT", "/api/v1/invoices/fiscal-settings", body, cookie, csrf, s.Core.Config.Origin, "fiscal-forged")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "next_value")
	status, out = purchaseOrderHTTP(t, h, "PUT", "/api/v1/invoices/fiscal-settings", body, cookie, "", s.Core.Config.Origin, "fiscal-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "PUT", "/api/v1/invoices/fiscal-settings", body, cookie, csrf, s.Core.Config.Origin, "fiscal-valid")
	if status != 403 {
		t.Fatal("reauth", status, out)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE user_id=?`, a.UserID); e != nil {
		t.Fatal(e)
	}
	status, out = purchaseOrderHTTP(t, h, "PUT", "/api/v1/invoices/fiscal-settings", body, cookie, csrf, s.Core.Config.Origin, "fiscal-valid")
	if status != 200 || fmt.Sprint(out["next_value"]) != "1" {
		t.Fatal(status, out)
	}
}
