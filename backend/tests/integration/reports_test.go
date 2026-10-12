package integration

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/expenses"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/reports"
	"io"
	"log/slog"
	"net/http"
	"reflect"
	"testing"
)

func grantReports(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	name := "reports-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var role string
	if e := s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, name).Scan(&role); e != nil {
		t.Fatal(e)
	}
	for _, p := range reports.Permissions {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, role, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, role); e != nil {
		t.Fatal(e)
	}
}
func reportsService(s *core.Service) *reports.Service {
	return &reports.Service{Core: s, Sources: map[string]bool{"invoices": true, "orders": true, "expenses": true}}
}
func reportFilter() reports.Filter { return reports.Filter{Page: 1, PerPage: 1} }
func TestReportsSourceIntersectionPaginationExactTotalsAndHome(t *testing.T) {
	s, a := expenseFixture(t)
	r := reportsService(s.Core)
	f := reportFilter()
	if _, e := r.Read(ctx, a, "expenses", f); e != core.Forbidden {
		t.Fatal(e)
	}
	grantReports(t, s.Core, a, "all")
	if _, e := r.Read(ctx, a, "expenses", f); e != core.Forbidden {
		t.Fatal(e)
	}
	h, e := r.Home(ctx, a, f)
	if e != nil || len(h.Reports) != 0 {
		t.Fatal(h, e)
	}
	grantExpense(t, s.Core, a, "all")
	makeExpense(t, s, a, "report-hnl-1", expenseInput())
	makeExpense(t, s, a, "report-hnl-2", expenseInput())
	p := expenseInput()
	p.Currency = "USD"
	p.Taxable = "8000000000000000.01"
	p.Exempt = "0"
	makeExpense(t, s, a, "report-usd", p)
	page, e := r.Read(ctx, a, "expenses", f)
	if e != nil || page.Total != 3 || len(page.Data) != 1 || len(page.Totals) != 2 || page.Totals[0].Values["total"] != "270.00" || page.Totals[1].Values["total"] != "9200000000000000.01" {
		t.Fatal(page, e)
	}
	f.Page = 2
	next, e := r.Read(ctx, a, "expenses", f)
	if e != nil || next.Data[0]["id"] == page.Data[0]["id"] || !reflect.DeepEqual(next.Totals, page.Totals) {
		t.Fatal(next, e)
	}
	h, e = r.Home(ctx, a, f)
	if e != nil || !reflect.DeepEqual(h.Reports["expenses"], page.Totals) || len(h.Reports) != 1 {
		t.Fatal(h, e)
	}
	f = reportFilter()
	f.From = "2026-10-12"
	f.To = f.From
	f.Currency = "USD"
	page, e = r.Read(ctx, a, "expenses", f)
	if e != nil || page.Total != 1 || page.Totals[0].Currency != "USD" {
		t.Fatal(page, e)
	}
	f.From = "2026-10-13"
	f.To = f.From
	page, e = r.Read(ctx, a, "expenses", f)
	if e != nil || page.Total != 0 || len(page.Totals) != 0 || len(page.Data) != 0 {
		t.Fatal(page, e)
	}
	r.Sources["expenses"] = false
	if _, e = r.Read(ctx, a, "expenses", reportFilter()); e != core.Missing {
		t.Fatal(e)
	}
}
func TestReportsScopesIntersectIndependentlyAndRevocation(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "self")
	makeExpense(t, s, a, "report-owned", expenseInput())
	other := supplierActor(t, s.Core, "self")
	grantExpense(t, s.Core, other, "all")
	makeExpense(t, s, other, "report-other", expenseInput())
	r := reportsService(s.Core)
	page, e := r.Read(ctx, a, "expenses", reportFilter())
	if e != nil || page.Total != 1 {
		t.Fatal(page, e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope='all' WHERE r.name='reports-self'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope='self' WHERE r.name='expenses-all'`); e != nil {
		t.Fatal(e)
	}
	page, e = r.Read(ctx, a, "expenses", reportFilter())
	if e != nil || page.Total != 1 {
		t.Fatal(page, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='gastos.view'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = r.Read(ctx, a, "expenses", reportFilter()); e != core.Forbidden {
		t.Fatal(e)
	}
	h, e := r.Home(ctx, a, reportFilter())
	if e != nil || len(h.Reports) != 0 {
		t.Fatal(h, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='reportes.view'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = r.Home(ctx, a, reportFilter()); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestReportsCanonicalFinancialsDatesAndExclusions(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantInvoice(t, s.Core, a, "all")
	grantPurchaseOrders(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "all")
	r := reportsService(s.Core)
	id := makeExpense(t, s, a, "report-trash-expense", expenseInput())
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.State(ctx, a, id, "report-trash", expenses.StateInput{Version: o.Version, Reason: "Duplicado"}, false); e != nil {
		t.Fatal(e)
	}
	po := &purchaseorders.Service{Core: s.Core}
	p := purchaseOrderInput("", "")
	p.Customer.Name = "Cliente reporte"
	v, e := po.Save(ctx, a, "", "report-order", p)
	oid := customerResult(t, v, e)["id"]
	orders, e := r.Read(ctx, a, "orders", reportFilter())
	stored, e2 := po.Get(ctx, a, oid)
	if e != nil || e2 != nil || orders.Total != 1 || orders.Data[0]["profit"] != stored.Calculation.Profit || orders.Data[0]["investment_total"] != stored.Calculation.InvestmentTotal || orders.Data[0]["subtotal"] != "180.00" {
		t.Fatal(orders, stored, e, e2)
	}
	f := reportFilter()
	f.From = "2026-10-11"
	page, e := r.Read(ctx, a, "orders", f)
	if e != nil || page.Total != 0 {
		t.Fatal(page, e)
	}
	inv := &invoices.Service{Core: s.Core}
	v, e = inv.Save(ctx, a, "", "report-invoice", invoiceInput())
	iid := customerResult(t, v, e)["id"]
	// State mutations below are synthetic DB fixtures only: prove read exclusions without fiscal infrastructure.
	if _, e = s.Core.DB.Exec(`UPDATE invoices SET status='cancelled' WHERE id=?`, iid); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET deleted_at=UTC_TIMESTAMP(6) WHERE id=?`, oid); e != nil {
		t.Fatal(e)
	}
	for _, kind := range []string{"invoices", "orders", "expenses"} {
		page, e = r.Read(ctx, a, kind, reportFilter())
		if e != nil || page.Total != 0 || len(page.Totals) != 0 {
			t.Fatal(kind, page, e)
		}
	}
}
func TestReportsTeamMembershipOnBothScopes(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "team")
	res, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Reportes equipo')`)
	if e != nil {
		t.Fatal(e)
	}
	n, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	team := core.WrapID(n)
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, a.UserID); e != nil {
		t.Fatal(e)
	}
	p := expenseInput()
	p.TeamID = team
	makeExpense(t, s, a, "report-team", p)
	makeExpense(t, s, a, "report-no-team", expenseInput())
	r := reportsService(s.Core)
	page, e := r.Read(ctx, a, "expenses", reportFilter())
	if e != nil || page.Total != 1 {
		t.Fatal(page, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE user_id=?`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = r.Read(ctx, a, "expenses", reportFilter()); e != core.Forbidden {
		t.Fatal(e)
	}
}

func TestReportsHTTPStrictQueriesAndSession(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "all")
	r := reportsService(s.Core)
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	reg := registry.New()
	if e := reg.Register(r.Module(logger, true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, reg, nil, logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	for _, path := range []string{"/reports/expenses?status=issued", "/reports/expenses?date_from=2026-02-30", "/reports/home?page=1", "/reports/orders?page=1&page=2", "/reports/invoices?status=cancelled"} {
		status, out := purchaseOrderHTTP(t, h, "GET", "/api/v1"+path, nil, cookie, csrf, s.Core.Config.Origin, "")
		if status != 422 {
			t.Fatal(path, status, out)
		}
	}
	status, _ := purchaseOrderHTTP(t, h, "GET", "/api/v1/reports/expenses", nil, nil, "", s.Core.Config.Origin, "")
	if status != 401 {
		t.Fatal(status)
	}
	status, out := purchaseOrderHTTP(t, h, "GET", "/api/v1/reports/expenses", nil, cookie, csrf, s.Core.Config.Origin, "")
	if status != 200 {
		t.Fatal(status, out)
	}
}

func TestReportsSnapshotTotalsStayConsistentWithConcurrentSourceWrites(t *testing.T) {
	s, a := expenseFixture(t)
	grantExpense(t, s.Core, a, "all")
	grantReports(t, s.Core, a, "all")
	id := makeExpense(t, s, a, "report-snapshot", expenseInput())
	r := reportsService(s.Core)
	done := make(chan struct{})
	errs := make(chan error, 1)
	go func() {
		defer close(done)
		for i := 0; i < 60; i++ {
			total := "135.00"
			if i%2 == 0 {
				total = "250.00"
			}
			if _, e := s.Core.DB.Exec(`UPDATE expenses SET total=? WHERE id=?`, total, id); e != nil {
				errs <- e
				return
			}
		}
	}()
	for i := 0; i < 15; i++ {
		page, e := r.Read(ctx, a, "expenses", reportFilter())
		if e != nil || page.Total != 1 || page.Totals[0].Values["total"] != page.Data[0]["total"] {
			t.Fatal(page, e)
		}
	}
	<-done
	select {
	case e := <-errs:
		t.Fatal(e)
	default:
	}
}
