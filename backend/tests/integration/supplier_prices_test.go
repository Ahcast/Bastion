package integration

import (
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func priceInput(version uint64) suppliers.PriceInput {
	return suppliers.PriceInput{SourceURL: "https://catalog.example.test/item/SKU-1", SourceType: "web", ObservedAt: time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC), PartNumber: "SKU-1", Price: "0012.345678", Currency: "usd", Availability: "Disponibilidad por confirmar", LeadTime: "7 días observados", MOQ: "10.25", Taxes: "unknown", Shipping: "excluded", Version: version}
}
func TestSupplierPricesEvidencePrecisionPaginationAndRedactedHistory(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	id := makeSupplier(t, s, a, "price-supplier", supplierInput("Proveedor precios"))
	v, e := s.RecordPrice(ctx, a, id, "price-observation", priceInput(1))
	if supplierResult(t, v, e)["version"] != "2" {
		t.Fatal(v)
	}
	if _, e = s.RecordPrice(ctx, a, id, "price-observation", priceInput(1)); e != nil {
		t.Fatal("replay", e)
	}
	changed := priceInput(1)
	changed.Price = "100"
	if _, e = s.RecordPrice(ctx, a, id, "price-observation", changed); e != core.Conflict {
		t.Fatal("different idempotent intent", e)
	}
	page, e := s.PriceHistory(ctx, a, id, "SKU-1", 1, 1)
	if e != nil || page.Total != 1 || len(page.Data) != 1 {
		t.Fatal(page, e)
	}
	p := page.Data[0]
	if p.Price != "12.345678" || p.MOQ != "10.2500" || p.Currency != "USD" || p.SourceURL == "" || p.UserName == "" || p.CapturedAt.IsZero() || p.ObservedAt.IsZero() || p.Shipping != "excluded" {
		t.Fatal("evidence or decimal lost", p)
	}
	for _, part := range []string{"%", "sku-1", "' OR 1=1 --"} {
		v, e := s.PriceHistory(ctx, a, id, part, 1, 25)
		if e != nil || v.Total != 0 {
			t.Fatal("literal exact part filter", v, e)
		}
	}
	c, e := s.Get(ctx, a, id)
	if e != nil || c.Name != "Proveedor precios" {
		t.Fatal("research overwrote master", c, e)
	}
	unknown := priceInput(c.Version)
	unknown.Price = ""
	unknown.Currency = ""
	unknown.PartNumber = ""
	unknown.SourceType = "catalog"
	if _, e = s.RecordPrice(ctx, a, id, "no-price-observation", unknown); e != nil {
		t.Fatal("evidence without price", e)
	}
	page, e = s.PriceHistory(ctx, a, id, "", 2, 1)
	if e != nil || page.Total != 2 || len(page.Data) != 1 {
		t.Fatal("price pagination", page, e)
	}
	history, e := s.History(ctx, a, id, 1, 100)
	if e != nil {
		t.Fatal(e)
	}
	encoded, _ := json.Marshal(history)
	for _, secret := range []string{"12.345678", "catalog.example.test", "Disponibilidad por confirmar"} {
		if strings.Contains(string(encoded), secret) {
			t.Fatal("price details copied to audit", string(encoded))
		}
	}
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='proveedores.view'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.PriceHistory(ctx, a, id, "", 1, 25); e != core.Forbidden {
		t.Fatal("price history bypassed revoked policy", e)
	}
}
func TestSupplierPriceRollbackIdempotencyAndConcurrentVersion(t *testing.T) {
	s, a, _ := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	id := makeSupplier(t, s, a, "concurrent-price-supplier", supplierInput("Proveedor concurrente"))
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER supplier_price_audit_failure BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic audit outage'`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.RecordPrice(ctx, a, id, "failed-price", priceInput(1)); e == nil {
		t.Fatal("audit failure swallowed")
	}
	var count int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM supplier_price_observations`).Scan(&count); e != nil {
		t.Fatal(e)
	}
	if count != 0 {
		t.Fatal("partial price committed")
	}
	c, e := s.Get(ctx, a, id)
	if e != nil || c.Version != 1 {
		t.Fatal("partial version committed", c, e)
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM domain_events WHERE topic='suppliers.price_observed'`).Scan(&count); e != nil {
		t.Fatal(e)
	}
	if count != 0 {
		t.Fatal("partial fact committed")
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM idempotency_records WHERE operation=?`, "suppliers.price:"+id).Scan(&count); e != nil {
		t.Fatal(e)
	}
	if count != 0 {
		t.Fatal("failed intent reserved")
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER supplier_price_audit_failure`); e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, e := s.RecordPrice(ctx, a, id, "failed-price", priceInput(1)); e != nil {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM supplier_price_observations`).Scan(&count); e != nil {
		t.Fatal(e)
	}
	if count != 1 {
		t.Fatal("idempotent price duplicated", count)
	}
	var success, conflict atomic.Int32
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			p := priceInput(2)
			p.Price = fmt.Sprint(i + 1)
			_, e := s.RecordPrice(ctx, a, id, fmt.Sprint("price-version-", i), p)
			if e == nil {
				success.Add(1)
			} else if e == core.Conflict {
				conflict.Add(1)
			} else {
				t.Error(e)
			}
		}(i)
	}
	wg.Wait()
	if success.Load() != 1 || conflict.Load() != 7 {
		t.Fatal("concurrent version lost", success.Load(), conflict.Load())
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM supplier_price_observations`).Scan(&count); e != nil {
		t.Fatal(e)
	}
	if count != 2 {
		t.Fatal("conflicting prices persisted", count)
	}
}
func TestSupplierPriceRESTMCPParityAndRestrictedReaders(t *testing.T) {
	s, a, r := suppliersFixture(t)
	grantSupplier(t, s.Core, a, "all")
	id := makeSupplier(t, s, a, "sdk-price-supplier", supplierInput("Proveedor SDK precios"))
	if _, e := s.RecordPrice(ctx, a, id, "sdk-price", priceInput(1)); e != nil {
		t.Fatal(e)
	}
	owner := supplierActor(t, s.Core, "self")
	if _, e := s.PriceHistory(ctx, owner, id, "", 1, 25); e != core.Missing {
		t.Fatal("price IDOR", e)
	}
	v, e := s.Core.CreateService(ctx, a, "Precios proveedores", []string{"proveedores.view"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	token := v.(map[string]any)["token"].(string)
	server := httptest.NewServer(api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil))))
	defer server.Close()
	client := mcp.NewClient(&mcp.Implementation{Name: "supplier-price-test", Version: "1"}, nil)
	session, e := client.Connect(ctx, &mcp.StreamableClientTransport{Endpoint: server.URL + "/mcp", HTTPClient: &http.Client{Transport: bearer{token}}}, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer session.Close()
	result, e := session.CallTool(ctx, &mcp.CallToolParams{Name: "get_supplier_price_history", Arguments: map[string]any{"id": id, "page": 1, "per_page": 25, "part_number": "SKU-1"}})
	if e != nil || result.IsError {
		t.Fatal("price MCP", result, e)
	}
	var output suppliers.PricePage
	b, _ := json.Marshal(result.StructuredContent)
	if e = json.Unmarshal(b, &output); e != nil || output.Total != 1 || output.Data[0].Price != "12.345678" {
		t.Fatal(string(b), e)
	}
	result, e = session.CallTool(ctx, &mcp.CallToolParams{Name: "resolve_supplier_exact", Arguments: map[string]any{"name": "Proveedor SDK precios"}})
	if e != nil || result.IsError {
		t.Fatal("exact supplier SDK", result, e)
	}
	actor, e := s.Core.AuthenticateService(ctx, token)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.RecordPrice(ctx, actor, id, "machine-price", priceInput(2)); e != core.Forbidden {
		t.Fatal("machine wrote evidence", e)
	}
	cookieToken, csrf, e := s.Core.NewSession(ctx, "1", true)
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: cookieToken}
	h := api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	status, response, _ := execute(t, h, "GET", "/api/v1/suppliers/"+id+"/price-history?part_number=SKU-1", nil, cookie, csrf, s.Core.Config.Origin)
	if status != 200 || response["total"].(float64) != 1 {
		t.Fatal("REST history", status, response)
	}
	status, _, _ = execute(t, h, "POST", "/api/v1/suppliers/"+id+"/price-history", map[string]any{"user_id": "1", "source_url": "https://example.test"}, cookie, csrf, s.Core.Config.Origin)
	if status != 422 {
		t.Fatal("forged evidence actor", status)
	}
	if e = s.Core.RevokeService(ctx, a, actor.ID); e != nil {
		t.Fatal(e)
	}
	result, e = session.CallTool(ctx, &mcp.CallToolParams{Name: "get_supplier_price_history", Arguments: map[string]any{"id": id, "page": 1, "per_page": 25}})
	if e == nil && !result.IsError {
		t.Fatal("revoked connection read prices")
	}
}
