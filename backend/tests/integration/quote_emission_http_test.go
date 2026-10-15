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
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

func TestQuoteEmissionStrictHTTPAndDisabledWorkflow(t *testing.T) {
	s, a, _, id, p := emissionFixture(t)
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	r := registry.New()
	if e := r.Register(s.ModuleFiles(logger, true, s.Store.(*pdfObjects))); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, s.Store.(*pdfObjects), logger)
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	if _, e = s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, security.Hash(token)); e != nil {
		t.Fatal(e)
	}
	for _, body := range []map[string]any{{"version": 4, "approval_id": p.ApprovalID}, {"version": "4", "approval_id": nil}, {"version": "4", "approval_id": p.ApprovalID, "status": "sent"}, {"version": "4"}} {
		status, _, _ := execute(t, h, "POST", "/api/v1/quotes/"+id+"/emit", body, cookie, csrf, s.Core.Config.Origin)
		if status != 422 {
			t.Fatal(status, body)
		}
	}
	raw, _ := json.Marshal(p)
	req := httptest.NewRequest("POST", "/api/v1/quotes/"+id+"/emit", bytes.NewReader(raw))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Origin", s.Core.Config.Origin)
	req.Header.Set("X-CSRF-Token", csrf)
	req.Header.Set("Idempotency-Key", "http-emission-success")
	req.AddCookie(cookie)
	result := httptest.NewRecorder()
	h.ServeHTTP(result, req)
	if result.Code != 200 {
		t.Fatal(result.Code, result.Body.String())
	}
	s.Approvals = nil
	r = registry.New()
	_ = r.Register(s.Module(logger, true))
	h = api.New(s.Core, r, nil, logger)
	status, _, _ := execute(t, h, "POST", "/api/v1/quotes/"+id+"/emit", map[string]any{"version": "4", "approval_id": p.ApprovalID}, cookie, csrf, s.Core.Config.Origin)
	if status != 404 {
		t.Fatal("disabled route", status)
	}
}
