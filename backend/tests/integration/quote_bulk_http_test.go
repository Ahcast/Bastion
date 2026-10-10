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

func TestQuoteBulkHTTPRoutesUseStrictDTOAndIdempotency(t *testing.T) {
	s, a, _ := quotesFixture(t)
	grantQuote(t, s.Core, a, "all")
	id := makeQuote(t, s, a, "bulk-http-source", quoteInput())
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

	invalid := map[string]any{"items": []map[string]any{{"id": id, "version": 1}}, "status": "costing"}
	status, _, _ := execute(t, h, "POST", "/api/v1/quotes/bulk/status", invalid, cookie, csrf, s.Core.Config.Origin)
	if status != 422 {
		t.Fatal("numeric version accepted", status)
	}

	body := map[string]any{"items": []map[string]any{{"id": id, "version": "1"}}, "status": "costing"}
	raw, _ := json.Marshal(body)
	req := httptest.NewRequest("POST", "/api/v1/quotes/bulk/status", bytes.NewReader(raw))
	req.RemoteAddr = "192.0.2.10:5000"
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Origin", s.Core.Config.Origin)
	req.Header.Set("X-CSRF-Token", csrf)
	req.Header.Set("Idempotency-Key", "bulk-http-status")
	req.AddCookie(cookie)
	w := httptest.NewRecorder()
	h.ServeHTTP(w, req)
	if w.Code != http.StatusOK {
		t.Fatal(w.Code, w.Body.String())
	}
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Status != "costing" || q.Version != 2 {
		t.Fatal(q, e)
	}
}
