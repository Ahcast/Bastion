package integration

import (
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
	"io"
	"log/slog"
	"net/http"
	"sync"
	"testing"
	"time"
)

func invoiceToday() string {
	return time.Now().In(time.FixedZone("Honduras", -6*3600)).Format("2006-01-02")
}
func invoiceEmissionFixture(t *testing.T) (*invoices.Service, core.Actor, string, invoices.EmitInput) {
	t.Helper()
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Empresa fiscal sintética","tax_id":"08011900123456","address":"Dirección sintética"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	p := syntheticFiscalSettings()
	today, _ := time.Parse("2006-01-02", invoiceToday())
	p.Value.ValidFrom = today.AddDate(0, 0, -1).Format("2006-01-02")
	p.Value.ValidUntil = today.AddDate(0, 0, 1).Format("2006-01-02")
	if _, e := s.SaveFiscalSettings(ctx, a, "emit-fiscal-create", p); e != nil {
		t.Fatal(e)
	}
	draft := invoiceInput()
	draft.InvoiceDate = invoiceToday()
	v, e := s.Save(ctx, a, "", "emit-draft-create", draft)
	id := customerResult(t, v, e)["id"]
	review, e := s.EmissionContext(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	return s, a, id, invoices.EmitInput{Version: review.InvoiceVersion, FiscalVersion: review.FiscalVersion, CompanyVersion: review.CompanyVersion, Reason: "Emisión sintética revisada"}
}
func assertInvoiceNotIssued(t *testing.T, s *invoices.Service, id string, next uint64) {
	t.Helper()
	var status string
	var number, sar, company *string
	var issued *time.Time
	var counter uint64
	if e := s.Core.DB.QueryRow(`SELECT status,invoice_number,CAST(sar_snapshot_json AS CHAR),CAST(company_snapshot_json AS CHAR),issued_at FROM invoices WHERE id=?`, id).Scan(&status, &number, &sar, &company, &issued); e != nil {
		t.Fatal(e)
	}
	if e := s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&counter); e != nil {
		t.Fatal(e)
	}
	if status != "draft" || number != nil || sar != nil || company != nil || issued != nil || counter != next {
		t.Fatal(status, number, counter)
	}
	var events int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events WHERE topic='invoice.issued' AND entity_id=?`, id).Scan(&events); e != nil || events != 0 {
		t.Fatal(events, e)
	}
}
func TestInvoiceEmissionRoundTripSnapshotsCreditAndReplay(t *testing.T) {
	s, a, id, p := invoiceEmissionFixture(t)
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	before := o.Items[0].ID
	input := editInvoice(o)
	days := uint16(30)
	input.PaymentTerm = "credito"
	input.CreditDays = &days
	v, e := s.Save(ctx, a, id, "emit-credit", input)
	_ = customerResult(t, v, e)
	review, e := s.EmissionContext(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p.Version = review.InvoiceVersion
	today, _ := time.Parse("2006-01-02", invoiceToday())
	if review.EstimatedNumber != "TEST-00000001" || review.DueDate != today.AddDate(0, 0, 30).Format("2006-01-02") {
		t.Fatal(review)
	}
	v, e = s.Emit(ctx, a, id, "emit-confirmed", p)
	result := customerResult(t, v, e)
	if result["invoice_number"] != "TEST-00000001" || result["status"] != "issued" || result["version"] != "3" {
		t.Fatal(result)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Version != 3 || o.Number == nil || *o.Number != result["invoice_number"] || o.Status != "issued" || o.Items[0].ID != before || o.IssuedAt == nil || o.IssuedBy == nil || *o.IssuedBy != a.UserID || o.DueDate == nil || *o.DueDate != review.DueDate || o.CompanyVersion == nil || *o.CompanyVersion != 1 || o.Balance != "201.25" || o.PaidTotal != "0.00" || o.Calculation.Profit != "85.01" {
		t.Fatal(o, e)
	}
	var snapshot invoices.FiscalSnapshot
	if e = json.Unmarshal(o.SAR, &snapshot); e != nil || snapshot.Version != 1 || snapshot.Configuration.CAI != "CAI-SINTETICO-NO-VALIDO" {
		t.Fatal(snapshot, e)
	}
	frozenSAR, frozenCompany := string(o.SAR), string(o.Company)
	settings, e := s.FiscalSettings(ctx, a)
	if e != nil {
		t.Fatal(e)
	}
	settings.Value.CAI = "CAI-CAMBIADO-SINTETICO"
	settings.Value.Enabled = false
	if _, e = s.SaveFiscalSettings(ctx, a, "emit-config-changed", invoices.FiscalSettingsInput{Version: settings.Version, Value: settings.Value, Reason: "Cambio posterior"}); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 1, Value: []byte(`{"name":"Empresa posterior","tax_id":"08011900123457","address":"Nueva dirección"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	v, e = s.Emit(ctx, a, id, "emit-confirmed", p)
	replay := customerResult(t, v, e)
	if replay["invoice_number"] != result["invoice_number"] || replay["version"] != result["version"] || replay["issued_at"] != result["issued_at"] {
		t.Fatal(replay, result)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || string(o.SAR) != frozenSAR || string(o.Company) != frozenCompany {
		t.Fatal(o, e)
	}
	if _, e = s.Save(ctx, a, id, "emit-no-edit", editInvoice(o)); e != core.Conflict {
		t.Fatal(e)
	}
	p.Reason = "Intención distinta"
	if _, e = s.Emit(ctx, a, id, "emit-confirmed", p); e != core.Conflict {
		t.Fatal(e)
	}
	rows, e := s.List(ctx, a, invoices.Filter{Page: 1, PerPage: 25, Status: "issued"})
	if e != nil || rows.Total != 1 || rows.Data[0].Number == nil {
		t.Fatal(rows, e)
	}
	var n uint64
	if e = s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&n); e != nil || n != 2 {
		t.Fatal(n, e)
	}
}
func TestInvoiceEmissionRejectsChangedVersionsAndFinancialEvidence(t *testing.T) {
	for _, change := range []string{"invoice", "fiscal", "company", "disabled", "expiry", "future", "currency", "header_calc", "line_calc", "reauth", "view", "emit"} {
		t.Run(change, func(t *testing.T) {
			s, a, id, p := invoiceEmissionFixture(t)
			var e error
			switch change {
			case "invoice", "future", "currency":
				o, err := s.Get(ctx, a, id)
				if err != nil {
					t.Fatal(err)
				}
				input := editInvoice(o)
				input.PublicNotes = "Revisión nueva"
				if change == "future" {
					d, _ := time.Parse("2006-01-02", invoiceToday())
					input.InvoiceDate = d.AddDate(0, 0, 1).Format("2006-01-02")
				}
				if change == "currency" {
					input.Currency = "USD"
				}
				_, e = s.Save(ctx, a, id, "emit-change", input)
				if change != "invoice" {
					p.Version = 2
				}
			case "fiscal", "disabled", "expiry":
				o, err := s.FiscalSettings(ctx, a)
				if err != nil {
					t.Fatal(err)
				}
				o.Value.CAI = "CAI-REVISADO-SINTETICO"
				if change == "disabled" {
					o.Value.Enabled = false
				}
				if change == "expiry" {
					d, _ := time.Parse("2006-01-02", invoiceToday())
					o.Value.ValidFrom = d.AddDate(0, 0, -2).Format("2006-01-02")
					o.Value.ValidUntil = d.AddDate(0, 0, -1).Format("2006-01-02")
				}
				_, e = s.SaveFiscalSettings(ctx, a, "emit-fiscal-change", invoices.FiscalSettingsInput{Version: o.Version, Value: o.Value, Reason: "Cambio sintético"})
				if change != "fiscal" {
					p.FiscalVersion = 2
				}
			case "company":
				e = s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 1, Value: []byte(`{"name":"Empresa revisada","tax_id":"08011900123456","address":"Dirección"}`)}, "")
			case "header_calc":
				_, e = s.Core.DB.Exec(`UPDATE invoices SET profit=0 WHERE id=?`, id)
			case "line_calc":
				_, e = s.Core.DB.Exec(`UPDATE invoice_items SET line_cost=0 WHERE invoice_id=?`, id)
			case "reauth":
				_, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash)
			case "view", "emit":
				_, e = s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission=?`, a.UserID, "facturas."+change)
			}
			if e != nil {
				t.Fatal(e)
			}
			_, e = s.Emit(ctx, a, id, "emit-rejected", p)
			if change == "reauth" {
				if ce, ok := e.(*core.Error); !ok || ce.Code != "reauth_required" {
					t.Fatal(e)
				}
			} else {
				want := core.Conflict
				if change == "view" {
					want = core.Missing
				}
				if change == "emit" {
					want = core.Forbidden
				}
				if e != want {
					t.Fatal(change, e, want)
				}
			}
			assertInvoiceNotIssued(t, s, id, 1)
		})
	}
}
func TestInvoiceEmissionConcurrentReplayUniqueNumbersAndRollback(t *testing.T) {
	s, a, id, p := invoiceEmissionFixture(t)
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_invoice_emit BEFORE INSERT ON domain_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic emission rollback'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "emit-rollback", p); e == nil {
		t.Fatal("emission committed despite event failure")
	}
	assertInvoiceNotIssued(t, s, id, 1)
	if _, e := s.Core.DB.Exec(`DROP TRIGGER reject_invoice_emit`); e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	errors := make(chan error, 12)
	numbers := make(chan string, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			v, e := s.Emit(ctx, a, id, "emit-rollback", p)
			if e == nil {
				raw, _ := json.Marshal(v)
				var r invoices.EmissionResult
				e = json.Unmarshal(raw, &r)
				numbers <- r.Number
			}
			errors <- e
		}()
	}
	wg.Wait()
	close(errors)
	close(numbers)
	for e := range errors {
		if e != nil {
			t.Fatal(e)
		}
	}
	for n := range numbers {
		if n != "TEST-00000001" {
			t.Fatal(n)
		}
	}
	var ids []string
	for i := 0; i < 6; i++ {
		draft := invoiceInput()
		draft.InvoiceDate = invoiceToday()
		draft.Customer.Name = fmt.Sprintf("Concurrente %d", i)
		v, e := s.Save(ctx, a, "", fmt.Sprintf("emit-new-%d", i), draft)
		ids = append(ids, customerResult(t, v, e)["id"])
	}
	errors = make(chan error, 6)
	numbers = make(chan string, 6)
	other := customerActor(t, s.Core, "all")
	grantInvoice(t, s.Core, other, "all")
	for i, id := range ids {
		actor := a
		if i%2 == 1 {
			actor = other
		}
		wg.Add(1)
		go func(id string, actor core.Actor) {
			defer wg.Done()
			v, e := s.Emit(ctx, actor, id, "emit-distinct", p)
			if e == nil {
				raw, _ := json.Marshal(v)
				var r invoices.EmissionResult
				e = json.Unmarshal(raw, &r)
				numbers <- r.Number
			}
			errors <- e
		}(id, actor)
	}
	wg.Wait()
	close(errors)
	close(numbers)
	for e := range errors {
		if e != nil {
			t.Fatal(e)
		}
	}
	seen := map[string]bool{}
	for n := range numbers {
		if seen[n] || n == "TEST-00000001" {
			t.Fatal(n)
		}
		seen[n] = true
	}
	if len(seen) != 6 {
		t.Fatal(seen)
	}
	var next, events uint64
	if e := s.Core.DB.QueryRow(`SELECT next_value FROM document_counters WHERE name='invoice'`).Scan(&next); e != nil || next != 8 {
		t.Fatal(next, e)
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events WHERE topic='invoice.issued'`).Scan(&events); e != nil || events != 7 {
		t.Fatal(events, e)
	}
	if _, e := s.Core.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='facturas.emit'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "emit-rollback", p); e != core.Forbidden {
		t.Fatal(e)
	}
}
func TestInvoiceEmissionHTTPStrictFieldsCSRFAndMissingCompany(t *testing.T) {
	s, a, id, p := invoiceEmissionFixture(t)
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
	for name := range body {
		clone := map[string]any{}
		for k, v := range body {
			clone[k] = v
		}
		delete(clone, name)
		status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices/"+id+"/emit", clone, cookie, csrf, s.Core.Config.Origin, "emit-missing")
		if status != 422 {
			t.Fatal(name, status, out)
		}
	}
	body["invoice_number"] = "TEST-00000099"
	status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices/"+id+"/emit", body, cookie, csrf, s.Core.Config.Origin, "emit-forged")
	if status != 422 {
		t.Fatal(status, out)
	}
	delete(body, "invoice_number")
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices/"+id+"/emit", body, cookie, "", s.Core.Config.Origin, "emit-no-csrf")
	if status != 403 {
		t.Fatal(status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices/"+id+"/emit", body, cookie, csrf, s.Core.Config.Origin, "emit-no-reauth")
	if status != 403 || out["code"] != "reauth_required" {
		t.Fatal(status, out)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM settings WHERE name='company'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Emit(ctx, a, id, "emit-missing-company", p); e != core.Disabled {
		t.Fatal(e)
	}
	assertInvoiceNotIssued(t, s, id, 1)
	if e = s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Empresa sin RTN","tax_id":"","address":"Dirección"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Emit(ctx, a, id, "emit-empty-tax", p); e != core.Invalid {
		t.Fatal(e)
	}
	assertInvoiceNotIssued(t, s, id, 1)
	if e = s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 1, Value: []byte(`{"name":"Empresa fiscal restaurada","tax_id":"08011900123456","address":"Dirección"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE user_id=?`, a.UserID); e != nil {
		t.Fatal(e)
	}
	body["company_version"] = "2"
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/invoices/"+id+"/emit", body, cookie, csrf, s.Core.Config.Origin, "emit-http-success")
	if status != 200 || out["invoice_number"] != "TEST-00000001" || out["status"] != "issued" {
		t.Fatal(status, out)
	}

}
func TestInvoiceEmissionScopesAndMixedTaxCanonicalEvidence(t *testing.T) {
	s, a, id, _ := invoiceEmissionFixture(t)
	other := customerActor(t, s.Core, "self")
	grantInvoice(t, s.Core, other, "self")
	if _, e := s.EmissionContext(ctx, other, id); e != core.Missing {
		t.Fatal(e)
	}
	teamResult, e := s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo emisión sintético')`)
	if e != nil {
		t.Fatal(e)
	}
	teamN, e := teamResult.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	team := core.WrapID(teamN)
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members VALUES(?,?)`, team, other.UserID); e != nil {
		t.Fatal(e)
	}
	grantInvoice(t, s.Core, other, "team")
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	input := editInvoice(o)
	input.TeamID = team
	input.Items = nil
	for _, tax := range []string{"isv15", "isv18", "exempt", "exonerated"} {
		input.Items = append(input.Items, invoices.ItemInput{Item: finance.Item{Description: "Fiscal mixta", Unit: "u", Quantity: "2.0000", Price: "100.000000", Cost: "30.005000", DiscountType: "percent", Discount: "10.0000", Tax: tax}})
	}
	input.DiscountType = "percent"
	input.Discount = "5.0000"
	input.Exoneration = "Autorización exoneración sintética"
	if _, e = s.Save(ctx, a, id, "emit-mixed", input); e != nil {
		t.Fatal(e)
	}
	review, e := s.EmissionContext(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	p := invoices.EmitInput{Version: review.InvoiceVersion, FiscalVersion: review.FiscalVersion, CompanyVersion: review.CompanyVersion, Reason: "Mixta revisada"}
	if _, e = s.Emit(ctx, other, id, "emit-mixed-final", p); e != nil {
		t.Fatal(e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Calculation.Total != "740.43" || o.Calculation.Profit != "433.97" {
		t.Fatal(o, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM team_members WHERE team_id=? AND user_id=?`, team, other.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Emit(ctx, other, id, "emit-mixed-final", p); e != core.Missing {
		t.Fatal("team revoked replay", e)
	}

}
