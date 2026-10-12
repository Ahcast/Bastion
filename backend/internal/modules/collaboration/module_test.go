package collaboration

import (
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func TestModuleRegistersDocumentedRoutes(t *testing.T) {
	service := &Service{Core: &core.Service{}}
	mux := http.NewServeMux()
	service.Module(slog.New(slog.NewTextHandler(io.Discard, nil)), true, nil).Routes(mux)
	for _, route := range []struct{ method, path string }{
		{"GET", "/api/v1/collaboration"},
		{"GET", "/api/v1/collaboration/folders"},
		{"GET", "/api/v1/collaboration/teams"},
		{"POST", "/api/v1/collaboration/folders"},
		{"PATCH", "/api/v1/collaboration/folders/1"},
		{"DELETE", "/api/v1/collaboration/folders/1"},
		{"POST", "/api/v1/collaboration/folders/1/restore"},
		{"POST", "/api/v1/collaboration"},
		{"GET", "/api/v1/collaboration/1"},
		{"PATCH", "/api/v1/collaboration/1"},
		{"DELETE", "/api/v1/collaboration/1"},
		{"POST", "/api/v1/collaboration/1/restore"},
		{"GET", "/api/v1/collaboration/1/versions"},
		{"GET", "/api/v1/collaboration/1/preview"},
		{"GET", "/api/v1/collaboration/1/download"},
	} {
		_, pattern := mux.Handler(httptest.NewRequest(route.method, route.path, nil))
		if pattern == "" {
			t.Errorf("route not registered: %s %s", route.method, route.path)
		}
	}
}

func TestRequiredJSONAndQueryRejectAmbiguousInput(t *testing.T) {
	valid := httptest.NewRequest("POST", "/", strings.NewReader("{\"scope\":\"company\",\"person_id\":\"\",\"team_id\":\"\",\"parent_id\":\"\",\"name\":\"Manuales\"}"))
	valid.Header.Set("Content-Type", "application/json")
	var input FolderInput
	if e := decodeRequired(valid, &input, "scope", "person_id", "team_id", "parent_id", "name"); e != nil || input.Name != "Manuales" {
		t.Fatal(input, e)
	}
	for _, raw := range []string{
		"{\"scope\":\"company\",\"person_id\":\"\",\"team_id\":\"\",\"parent_id\":\"\"}",
		"{\"scope\":null,\"person_id\":\"\",\"team_id\":\"\",\"parent_id\":\"\",\"name\":\"Manuales\"}",
		"{\"scope\":\"company\",\"person_id\":\"\",\"team_id\":\"\",\"parent_id\":\"\",\"name\":\"Manuales\",\"deleted\":true}",
	} {
		r := httptest.NewRequest("POST", "/", strings.NewReader(raw))
		r.Header.Set("Content-Type", "application/json")
		if e := decodeRequired(r, &input, "scope", "person_id", "team_id", "parent_id", "name"); e != core.Invalid {
			t.Fatalf("accepted invalid body %s: %v", raw, e)
		}
	}
	oversized := httptest.NewRequest("POST", "/", strings.NewReader(`{"name":"`+strings.Repeat("x", (1<<20)+1)+`"}`))
	oversized.Header.Set("Content-Type", "application/json")
	if e := decodeRequired(oversized, &input, "name"); e != core.Invalid {
		t.Fatalf("accepted oversized body: %v", e)
	}
	for _, target := range []string{"/?scope=company&scope=person", "/?unexpected=true"} {
		r := httptest.NewRequest("GET", target, nil)
		if _, e := queryValues(r, "scope"); e != core.Invalid {
			t.Fatalf("accepted ambiguous query %s: %v", target, e)
		}
	}
}
