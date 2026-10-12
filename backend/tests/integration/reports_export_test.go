package integration

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"encoding/xml"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/reports"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
)

func reportPart(t *testing.T, data []byte, name string) []byte {
	t.Helper()
	z, e := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if e != nil {
		t.Fatal(e)
	}
	for _, f := range z.File {
		if f.Name == name {
			r, e := f.Open()
			if e != nil {
				t.Fatal(e)
			}
			defer r.Close()
			b, e := io.ReadAll(r)
			if e != nil {
				t.Fatal(e)
			}
			return b
		}
	}
	t.Fatal(name)
	return nil
}
func exportIDs(t *testing.T, data []byte) []string {
	t.Helper()
	var s struct {
		Rows []struct {
			Number int `xml:"r,attr"`
			Cells  []struct {
				Ref  string `xml:"r,attr"`
				Text string `xml:"is>t"`
			} `xml:"c"`
		} `xml:"sheetData>row"`
	}
	if e := xml.Unmarshal(reportPart(t, data, "xl/worksheets/sheet1.xml"), &s); e != nil {
		t.Fatal(e)
	}
	out := []string{}
	for _, r := range s.Rows {
		if r.Number > 6 {
			for _, c := range r.Cells {
				if c.Ref == "A"+strconv.Itoa(r.Number) {
					out = append(out, c.Text)
				}
			}
		}
	}
	return out
}
func TestReportsExportReauthIndependentScopeRevocationAndAudit(t *testing.T) {
	s, a := expenseFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "all")
	po := &purchaseorders.Service{Core: s.Core}
	p := purchaseOrderInput("", "")
	p.Customer.Name = "=Cliente protegido"
	v, e := po.Save(ctx, a, "", "export-order", p)
	id := customerResult(t, v, e)["id"]
	other := supplierActor(t, s.Core, "self")
	grantPurchaseOrders(t, s.Core, other, "all")
	v, e = po.Save(ctx, other, "", "export-other", p)
	otherID := customerResult(t, v, e)["id"]
	r := reportsService(s.Core)
	f := reportFilter()
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = r.ExportOrders(ctx, a, f); e == nil || e.(*core.Error).Code != "reauth_required" {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles ro ON ro.id=rp.role_id SET rp.scope='self' WHERE ro.name='reports-all' AND rp.permission='reportes.export'`); e != nil {
		t.Fatal(e)
	}
	out, e := r.ExportOrders(ctx, a, f)
	if e != nil {
		t.Fatal(e)
	}
	ids := exportIDs(t, out.Data)
	if len(ids) != 1 || ids[0] != id {
		t.Fatal(ids, otherID)
	}
	var count int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM audit_events WHERE action='reports.orders.export'`).Scan(&count); e != nil || count != 1 {
		t.Fatal(count, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='reportes.export'`); e != nil {
		t.Fatal(e)
	}
	if _, e = r.ExportOrders(ctx, a, f); e != core.Forbidden {
		t.Fatal(e)
	}
	grantReports(t, s.Core, a, "all")
	big := p
	big.Currency = "USD"
	big.Items = append([]purchaseorders.Item{}, p.Items...)
	big.Items[0].Quantity = "10000"
	big.Items[0].UnitCost = "800000000000.000001"
	v, e = po.Save(ctx, a, "", "export-large-negative", big)
	bigID := customerResult(t, v, e)["id"]
	stored, e := po.Get(ctx, a, bigID)
	if e != nil {
		t.Fatal(e)
	}
	exact, e := r.ExportOrders(ctx, a, reports.Filter{Currency: "USD"})
	if e != nil {
		t.Fatal(e)
	}
	sheet := string(reportPart(t, exact.Data, "xl/worksheets/sheet1.xml"))
	totalsSheet := string(reportPart(t, exact.Data, "xl/worksheets/sheet2.xml"))
	if !strings.Contains(sheet, stored.Calculation.Profit) || !strings.Contains(sheet, stored.Calculation.InvestmentTotal) || !strings.Contains(totalsSheet, "Fuera de precisión Excel") {
		t.Fatal("large values lost")
	}
	if _, e = s.Core.DB.Exec(`CREATE TRIGGER export_audit_failure BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic audit failure'`); e != nil {
		t.Fatal(e)
	}
	out, e = r.ExportOrders(ctx, a, f)
	if e == nil || len(out.Data) != 0 {
		t.Fatal("audit rollback", e)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER export_audit_failure`); e != nil {
		t.Fatal(e)
	}
	f.From = "2030-01-01"
	out, e = r.ExportOrders(ctx, a, f)
	if e != nil || len(exportIDs(t, out.Data)) != 0 {
		t.Fatal(e)
	}
	r.Sources["orders"] = false
	if _, e = r.ExportOrders(ctx, a, reportFilter()); e != core.Missing {
		t.Fatal(e)
	}
	a.Type = "service"
	if _, e = r.ExportOrders(ctx, a, reportFilter()); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestReportsExportAllFilteredRowsAndExplicitLimit(t *testing.T) {
	s, a := expenseFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "all")
	po := &purchaseorders.Service{Core: s.Core}
	p := purchaseOrderInput("", "")
	p.Customer.Name = "Cliente exportado"
	v, e := po.Save(ctx, a, "", "export-many", p)
	customerResult(t, v, e)
	r := reportsService(s.Core)
	rows, e := s.Core.DB.Query(`SHOW COLUMNS FROM customer_purchase_orders`)
	if e != nil {
		t.Fatal(e)
	}
	cols := []string{}
	for rows.Next() {
		var field, typ, null, key, extra string
		var def any
		if e = rows.Scan(&field, &typ, &null, &key, &def, &extra); e != nil {
			t.Fatal(e)
		}
		if field != "id" {
			cols = append(cols, "`"+field+"`")
		}
	}
	if e = rows.Err(); e != nil {
		t.Fatal(e)
	}
	rows.Close()
	columns := strings.Join(cols, ",")
	// Synthetic fixtures clone stored values only, preserving all constraints and NULL source links.
	for i := 0; i < 7; i++ {
		if _, e = s.Core.DB.Exec(`INSERT INTO customer_purchase_orders (` + columns + `) SELECT ` + columns + ` FROM customer_purchase_orders`); e != nil {
			t.Fatal(e)
		}
	}
	page, e := r.Read(ctx, a, "orders", reportFilter())
	if e != nil || page.Total != 128 || len(page.Data) != 1 {
		t.Fatal(page.Total, e)
	}
	out, e := r.ExportOrders(ctx, a, reportFilter())
	if e != nil || len(exportIDs(t, out.Data)) != 128 {
		t.Fatal(e)
	}
	for i := 0; i < 7; i++ {
		if _, e = s.Core.DB.Exec(`INSERT INTO customer_purchase_orders (` + columns + `) SELECT ` + columns + ` FROM customer_purchase_orders`); e != nil {
			t.Fatal(e)
		}
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM customer_purchase_orders ORDER BY id DESC LIMIT 6383`); e != nil {
		t.Fatal(e)
	}
	out, e = r.ExportOrders(ctx, a, reportFilter())
	if e == nil || e.(*core.Error).Code != "export_too_large" || len(out.Data) != 0 {
		t.Fatal(e)
	}
}
func exportHTTP(t *testing.T, h http.Handler, body any, cookie *http.Cookie, csrf string) *httptest.ResponseRecorder {
	t.Helper()
	raw, _ := json.Marshal(body)
	r := httptest.NewRequest("POST", "/api/v1/reports/orders/export", bytes.NewReader(raw))
	r.RemoteAddr = "192.0.2.20:5000"
	r.Header.Set("Content-Type", "application/json")
	r.Header.Set("Origin", "http://localhost:4200")
	if cookie != nil {
		r.AddCookie(cookie)
	}
	if csrf != "" {
		r.Header.Set("X-CSRF-Token", csrf)
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	return w
}
func TestReportsExportHTTPStreamStrictDTOAndCSRF(t *testing.T) {
	s, a := expenseFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "all")
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	reg := registry.New()
	_ = reg.Register(reportsService(s.Core).Module(logger, true))
	h := api.New(s.Core, reg, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, security.Hash(token)); e != nil {
		t.Fatal(e)
	}
	body := map[string]any{"date_from": "", "date_to": "", "currency": "", "status": ""}
	w := exportHTTP(t, h, body, cookie, csrf)
	if w.Code != 200 || !strings.HasPrefix(w.Header().Get("Content-Type"), "application/vnd.openxmlformats") || w.Header().Get("Cache-Control") != "no-store" || w.Header().Get("Content-Disposition") != "attachment; filename=ordenes.xlsx" {
		t.Fatal(w.Code, w.Header(), w.Body.String())
	}
	exportIDs(t, w.Body.Bytes())
	if w = exportHTTP(t, h, body, cookie, ""); w.Code != 403 {
		t.Fatal(w.Code)
	}
	if w = exportHTTP(t, h, body, nil, csrf); w.Code != 401 {
		t.Fatal(w.Code)
	}
	for _, bad := range []map[string]any{{}, {"date_from": "", "date_to": "", "currency": "", "status": nil}, {"date_from": "", "date_to": "", "currency": "", "status": "", "page": 1}, {"date_from": "2026-02-30", "date_to": "", "currency": "", "status": ""}} {
		w = exportHTTP(t, h, bad, cookie, csrf)
		if w.Code != 422 || !strings.HasPrefix(w.Header().Get("Content-Type"), "application/json") {
			t.Fatal(w.Code, w.Body.String())
		}
	}
}
