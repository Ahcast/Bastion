package integration

import (
	"bytes"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
)

func purchaseOrderHTTP(t *testing.T, h http.Handler, method, path string, body any, cookie *http.Cookie, csrf, origin, key string) (int, map[string]any) {
	t.Helper()
	raw, e := json.Marshal(body)
	if e != nil {
		t.Fatal(e)
	}
	req := httptest.NewRequest(method, path, bytes.NewReader(raw))
	req.RemoteAddr = "192.0.2.10:5000"
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Origin", origin)
	if csrf != "" {
		req.Header.Set("X-CSRF-Token", csrf)
	}
	if key != "" {
		req.Header.Set("Idempotency-Key", key)
	}
	if cookie != nil {
		req.AddCookie(cookie)
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, req)
	var out map[string]any
	_ = json.Unmarshal(w.Body.Bytes(), &out)
	return w.Code, out
}

func TestPurchaseOrderHTTPStrictContract(t *testing.T) {
	s, _, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
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

	calc := map[string]any{
		"items": []map[string]any{{
			"description": "Equipo", "unit": "unidad", "quantity": "2", "unit_sale_price": "100", "unit_cost": "30.005",
			"discount_type": "amount", "discount": "10", "tax_type": "isv15",
		}},
		"general_discount_type": "amount", "general_discount": "0", "exoneration_reference": "", "additional_expenses": "9.99",
	}
	status, out := purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders/calculate", calc, cookie, csrf, s.Core.Config.Origin, "")
	if status != http.StatusOK {
		t.Fatal("calculate", status, out)
	}
	calculation, ok := out["calculation"].(map[string]any)
	if !ok || calculation["total"] != "207.00" || calculation["investment_total"] != "70.00" {
		t.Fatal("calculate result", out)
	}

	p := purchaseOrderInput("", "")
	p.RequiredDeliveryDate = nil
	status, _ = purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders", p, cookie, csrf, s.Core.Config.Origin, "")
	if status != http.StatusUnprocessableEntity {
		t.Fatal("missing idempotency accepted", status)
	}

	raw, e := json.Marshal(p)
	if e != nil {
		t.Fatal(e)
	}
	var missing map[string]any
	if e = json.Unmarshal(raw, &missing); e != nil {
		t.Fatal(e)
	}
	delete(missing, "required_delivery_date")
	status, _ = purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders", missing, cookie, csrf, s.Core.Config.Origin, "po-http-missing")
	if status != http.StatusUnprocessableEntity {
		t.Fatal("omitted nullable field accepted", status)
	}

	var unknown map[string]any
	if e = json.Unmarshal(raw, &unknown); e != nil {
		t.Fatal(e)
	}
	unknown["unexpected"] = true
	status, _ = purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders", unknown, cookie, csrf, s.Core.Config.Origin, "po-http-unknown")
	if status != http.StatusUnprocessableEntity {
		t.Fatal("unknown field accepted", status)
	}

	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders", p, cookie, csrf, s.Core.Config.Origin, "po-http-create")
	if status != http.StatusOK {
		t.Fatal("create", status, out)
	}
	id, ok := out["id"].(string)
	if !ok || id == "" || out["version"] != "1" {
		t.Fatal("create response", out)
	}

	status, out, _ = execute(t, h, "GET", "/api/v1/purchase-orders/"+id, nil, cookie, "", s.Core.Config.Origin)
	if status != http.StatusOK || out["order_number"] != p.OrderNumber || out["required_delivery_date"] != nil {
		t.Fatal("get", status, out)
	}
	status, out, _ = execute(t, h, "GET", "/api/v1/purchase-orders?page=1&per_page=25&status=received", nil, cookie, "", s.Core.Config.Origin)
	if status != http.StatusOK || out["total"].(float64) != 1 {
		t.Fatal("list", status, out)
	}
	status, _, _ = execute(t, h, "GET", "/api/v1/purchase-orders?page=0", nil, cookie, "", s.Core.Config.Origin)
	if status != http.StatusUnprocessableEntity {
		t.Fatal("invalid page accepted", status)
	}

	current, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	edit := editPurchaseOrder(current)
	edit.Comments = "HTTP actualizado"
	status, out = purchaseOrderHTTP(t, h, "PATCH", "/api/v1/purchase-orders/"+id, edit, cookie, csrf, s.Core.Config.Origin, "po-http-update")
	if status != http.StatusOK || out["version"] != "2" {
		t.Fatal("patch", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "PATCH", "/api/v1/purchase-orders/"+id, edit, cookie, csrf, s.Core.Config.Origin, "po-http-update")
	if status != http.StatusOK || out["version"] != "2" {
		t.Fatal("patch replay", status, out)
	}
	status, _ = purchaseOrderHTTP(t, h, "PATCH", "/api/v1/purchase-orders/"+id, edit, cookie, csrf, s.Core.Config.Origin, "po-http-stale")
	if status != http.StatusConflict {
		t.Fatal("stale patch accepted", status)
	}
	transitionPath := "/api/v1/purchase-orders/" + id + "/transition"
	for _, invalid := range []map[string]any{
		{"to_status": "confirmed"},
		{"to_status": "confirmed", "version": nil},
		{"to_status": "confirmed", "version": 2},
		{"to_status": "confirmed", "version": "2", "extra": true},
	} {
		status, _ = purchaseOrderHTTP(t, h, "POST", transitionPath, invalid, cookie, csrf, s.Core.Config.Origin, "po-http-transition")
		if status != http.StatusUnprocessableEntity {
			t.Fatal("invalid transition accepted", status)
		}
	}
	body := map[string]any{"to_status": "confirmed", "version": "2"}
	status, _ = purchaseOrderHTTP(t, h, "POST", transitionPath, body, cookie, csrf, s.Core.Config.Origin, "")
	if status != http.StatusUnprocessableEntity {
		t.Fatal("transition without key", status)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", transitionPath, body, cookie, csrf, s.Core.Config.Origin, "po-http-transition")
	if status != http.StatusOK || out["status"] != "confirmed" || out["version"] != "3" {
		t.Fatal("HTTP transition", status, out)
	}
	status, out = purchaseOrderHTTP(t, h, "POST", transitionPath, body, cookie, csrf, s.Core.Config.Origin, "po-http-transition")
	if status != http.StatusOK || out["version"] != "3" {
		t.Fatal("HTTP replay", status, out)
	}
	for _, position := range []string{"0", "201", "bad"} {
		status, _ = purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders/"+id+"/items/"+position+"/transition", map[string]any{"to_status": "purchased", "version": "3"}, cookie, csrf, s.Core.Config.Origin, "po-http-item")
		if status != http.StatusUnprocessableEntity {
			t.Fatal("invalid position", position, status)
		}
	}
	status, out = purchaseOrderHTTP(t, h, "POST", "/api/v1/purchase-orders/"+id+"/items/1/transition", map[string]any{"to_status": "purchased", "version": "3"}, cookie, csrf, s.Core.Config.Origin, "po-http-item")
	if status != http.StatusOK || out["status"] != "confirmed" || out["version"] != "4" {
		t.Fatal("HTTP item transition", status, out)
	}
	status, out, _ = execute(t, h, "GET", "/api/v1/purchase-orders/"+id+"/status-history?page=1&per_page=1", nil, cookie, "", s.Core.Config.Origin)
	if status != http.StatusOK || out["total"] != float64(2) {
		t.Fatal("HTTP history", status, out)
	}
	status, out, _ = execute(t, h, "GET", "/api/v1/purchase-orders?status=confirmed", nil, cookie, "", s.Core.Config.Origin)
	if status != http.StatusOK || out["total"] != float64(1) {
		t.Fatal("new status filter", status, out)
	}

}
