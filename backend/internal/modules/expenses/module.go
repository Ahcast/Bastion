package expenses

import (
	"bytes"
	"context"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"io"
	"log/slog"
	"net/http"
	"strconv"
)

func decode(r *http.Request, p any, names ...string) error {
	b, e := io.ReadAll(r.Body)
	if e != nil {
		return core.Invalid
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(b, &fields) != nil {
		return core.Invalid
	}
	for _, n := range names {
		v, ok := fields[n]
		if !ok || bytes.Equal(bytes.TrimSpace(v), []byte("null")) {
			return core.Invalid
		}
	}
	if raw, ok := fields["supplier"]; ok {
		var supplier map[string]json.RawMessage
		if json.Unmarshal(raw, &supplier) != nil {
			return core.Invalid
		}
		for _, n := range []string{"id", "name"} {
			v, ok := supplier[n]
			if !ok || bytes.Equal(bytes.TrimSpace(v), []byte("null")) {
				return core.Invalid
			}
		}
	}
	r.Body = io.NopCloser(bytes.NewReader(b))
	return api.Decode(r, p)
}
func filter(r *http.Request) (Filter, error) {
	v := r.URL.Query()
	f := Filter{Search: v.Get("search"), SupplierID: v.Get("supplier_id"), From: v.Get("date_from"), To: v.Get("date_to"), Currency: v.Get("currency"), Category: v.Get("category"), Page: 1, PerPage: 25}
	allowed := map[string]bool{}
	for _, x := range []string{"search", "supplier_id", "date_from", "date_to", "currency", "category", "deleted", "page", "per_page"} {
		allowed[x] = true
	}
	for k, x := range v {
		if !allowed[k] || len(x) != 1 {
			return f, core.Invalid
		}
	}
	var e error
	if v.Has("deleted") {
		if v.Get("deleted") != "true" && v.Get("deleted") != "false" {
			return f, core.Invalid
		}
		f.Deleted = v.Get("deleted") == "true"
	}
	if v.Has("page") {
		f.Page, e = strconv.Atoi(v.Get("page"))
		if e != nil {
			return f, core.Invalid
		}
	}
	if v.Has("per_page") {
		f.PerPage, e = strconv.Atoi(v.Get("per_page"))
		if e != nil {
			return f, core.Invalid
		}
	}
	return f, nil
}
func documentPage(r *http.Request) (int, int, error) {
	v := r.URL.Query()
	for k, values := range v {
		if (k != "page" && k != "per_page") || len(values) != 1 {
			return 0, 0, core.Invalid
		}
	}
	page, perPage := 1, 25
	var e error
	if v.Has("page") {
		page, e = strconv.Atoi(v.Get("page"))
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if v.Has("per_page") {
		perPage, e = strconv.Atoi(v.Get("per_page"))
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	return page, perPage, nil
}
func noQuery(r *http.Request) error {
	if len(r.URL.Query()) != 0 {
		return core.Invalid
	}
	return nil
}
func safe(e error) error {
	if e == nil || core.IsClient(e) {
		return e
	}
	return &core.Error{Code: "internal", Message: "No se pudo completar la operación.", Status: 500}
}
func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return registry.Module{ID: "expenses", Label: "Gastos", Version: "0.2.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Routes: func(mux *http.ServeMux) {
		register := func(path string, f api.Action) {
			mux.Handle(path, api.Operation(s.Core, logger, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				v, e := f(ctx, a, r)
				return v, safe(e)
			}))
		}
		register("GET /api/v1/expenses", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.List(ctx, a, f)
		})
		register("GET /api/v1/expenses/totals", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.Totals(ctx, a, f)
		})
		register("GET /api/v1/expenses/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := noQuery(r); e != nil {
				return nil, e
			}
			return s.Get(ctx, a, r.PathValue("id"))
		})
		register("GET /api/v1/expenses/{id}/duplicates", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.Duplicates(ctx, a, r.PathValue("id"), f.Page, f.PerPage)
		})
		register("GET /api/v1/expenses/{id}/references", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := noQuery(r); e != nil {
				return nil, e
			}
			return s.References(ctx, a, r.PathValue("id"))
		})
		register("POST /api/v1/expenses/{id}/references", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := noQuery(r); e != nil {
				return nil, e
			}
			var p SetReferencesInput
			if e := decode(r, &p, "version", "reason", "references"); e != nil {
				return nil, e
			}
			return s.SetReferences(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("GET /api/v1/expenses/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			page, perPage, e := documentPage(r)
			if e != nil {
				return nil, e
			}
			return s.Documents(ctx, a, r.PathValue("id"), page, perPage)
		})
		register("POST /api/v1/expenses/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := noQuery(r); e != nil {
				return nil, e
			}
			var p LinkDocumentInput
			if e := decode(r, &p, "version", "document_id", "document_version"); e != nil {
				return nil, e
			}
			return s.LinkDocument(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("POST /api/v1/expenses/calculate", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := s.gate(ctx, s.Core.DB, a, "gastos.view"); e != nil {
				return nil, e
			}
			var p CalculationInput
			if e := decode(r, &p, "currency", "taxable_amount", "exempt_amount"); e != nil {
				return nil, e
			}
			return Calculate(p)
		})
		for _, method := range []string{"POST", "PATCH"} {
			path := "/api/v1/expenses"
			if method == "PATCH" {
				path += "/{id}"
			}
			register(method+" "+path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p Input
				if e := decode(r, &p, "invoice_number", "expense_date", "supplier", "category", "currency", "taxable_amount", "exempt_amount", "notes", "team_id", "version"); e != nil {
					return nil, e
				}
				return s.Save(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
		}
		for _, restore := range []bool{false, true} {
			path := "DELETE /api/v1/expenses/{id}"
			if restore {
				path = "POST /api/v1/expenses/{id}/restore"
			}
			register(path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p StateInput
				if e := decode(r, &p, "version", "reason"); e != nil {
					return nil, e
				}
				return s.State(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p, restore)
			})
		}
	}}
}
