package suppliers

import (
	"bytes"
	"context"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"strconv"
)

func page(r *http.Request) (int, int, error) {
	p, l := 1, 25
	var e error
	v := r.URL.Query()
	if v.Get("page") != "" {
		p, e = strconv.Atoi(v.Get("page"))
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if v.Get("per_page") != "" {
		l, e = strconv.Atoi(v.Get("per_page"))
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if p < 1 || p > 100000 || l < 1 || l > 100 {
		return 0, 0, core.Invalid
	}
	return p, l, nil
}
func filter(r *http.Request) (Filter, error) {
	p, l, e := page(r)
	if e != nil {
		return Filter{}, e
	}
	v := r.URL.Query()
	f := Filter{Search: v.Get("search"), Country: v.Get("country"), Page: p, PerPage: l}
	if v.Has("is_active") {
		if v.Get("is_active") != "true" && v.Get("is_active") != "false" {
			return f, core.Invalid
		}
		x := v.Get("is_active") == "true"
		f.Active = &x
	}
	return f, nil
}
func safeError(e error) error {
	if e == nil || core.IsClient(e) {
		return e
	}
	return &core.Error{Code: "internal", Message: "No se pudo completar la operación.", Status: 500}
}

// PATCH replaces the complete editable aggregate. Reject missing/null fields rather
// than silently erasing data through zero values. Unknown fields stay forbidden.
func decodeInput(r *http.Request, p *Input) error {
	body, e := io.ReadAll(r.Body)
	if e != nil {
		return core.Invalid
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(body, &fields) != nil {
		return core.Invalid
	}
	for _, name := range []string{"name", "rtn", "contact_name", "phone", "email", "address", "country", "website", "notes", "payment_terms", "lead_time", "reliability", "is_active", "team_id", "version", "contacts", "products", "categories", "brands"} {
		v, ok := fields[name]
		if !ok || bytes.Equal(bytes.TrimSpace(v), []byte("null")) {
			return core.Invalid
		}
	}
	for _, name := range []string{"evaluation", "response_hours"} {
		if _, ok := fields[name]; !ok {
			return core.Invalid
		}
	}
	r.Body = io.NopCloser(bytes.NewReader(body))
	return api.Decode(r, p)
}

func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return registry.Module{ID: "suppliers", Label: "Proveedores", Version: "0.1.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Routes: func(mux *http.ServeMux) {
		register := func(pattern string, f api.Action) { mux.Handle(pattern, api.Operation(s.Core, logger, f)) }
		register("GET /api/v1/suppliers", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.List(ctx, a, f)
		})
		register("GET /api/v1/suppliers/teams", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) { return s.Teams(ctx, a) })
		register("GET /api/v1/suppliers/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			return s.Get(ctx, a, r.PathValue("id"))
		})
		for _, method := range []string{"POST", "PATCH"} {
			path := "/api/v1/suppliers"
			if method == "PATCH" {
				path += "/{id}"
			}
			register(method+" "+path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p Input
				if e := decodeInput(r, &p); e != nil {
					return nil, e
				}
				return s.Save(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
		}
		register("GET /api/v1/suppliers/{id}/notes", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			p, l, e := page(r)
			if e != nil {
				return nil, e
			}
			return s.Notes(ctx, a, r.PathValue("id"), p, l)
		})
		register("POST /api/v1/suppliers/{id}/notes", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p struct {
				Body    string `json:"body"`
				Version uint64 `json:"version,string"`
			}
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.AddNote(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Body, p.Version)
		})
		register("GET /api/v1/suppliers/{id}/history", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			p, l, e := page(r)
			if e != nil {
				return nil, e
			}
			return s.History(ctx, a, r.PathValue("id"), p, l)
		})
		register("GET /api/v1/suppliers/{id}/price-history", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			p, l, e := page(r)
			if e != nil {
				return nil, e
			}
			return s.PriceHistory(ctx, a, r.PathValue("id"), r.URL.Query().Get("part_number"), p, l)
		})
		register("POST /api/v1/suppliers/{id}/price-history", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p PriceInput
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.RecordPrice(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("GET /api/v1/suppliers/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			p, l, e := page(r)
			if e != nil {
				return nil, e
			}
			return s.Documents(ctx, a, r.PathValue("id"), p, l)
		})
		register("POST /api/v1/suppliers/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p struct {
				Document string `json:"document_id"`
				Version  uint64 `json:"version,string"`
			}
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.LinkDocument(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Document, p.Version)
		})
	}, Tools: func(server *mcp.Server) {
		mcp.AddTool(server, &mcp.Tool{Name: "search_suppliers", Description: "Buscar proveedores autorizados con filtros y paginación.", OutputSchema: outputSchema("list")}, func(ctx context.Context, req *mcp.CallToolRequest, f Filter) (*mcp.CallToolResult, List, error) {
			a, e := api.ActorFrom(ctx)
			if e != nil {
				return nil, List{}, e
			}
			if f.Page == 0 {
				f.Page = 1
			}
			if f.PerPage == 0 {
				f.PerPage = 25
			}
			v, e := s.List(ctx, a, f)
			if e == nil {
				e = s.Core.Audit(ctx, s.Core.DB, a, "mcp.search_suppliers", "suppliers", "", "success", map[string]int{"returned": len(v.Data)})
			}
			return nil, v, safeError(e)
		})
		mcp.AddTool(server, &mcp.Tool{Name: "get_supplier", Description: "Consultar una ficha de proveedor autorizada.", OutputSchema: outputSchema("supplier")}, func(ctx context.Context, req *mcp.CallToolRequest, p struct {
			ID string `json:"id"`
		}) (*mcp.CallToolResult, Supplier, error) {
			a, e := api.ActorFrom(ctx)
			if e != nil {
				return nil, Supplier{}, e
			}
			v, e := s.Get(ctx, a, p.ID)
			if e == nil {
				e = s.Core.Audit(ctx, s.Core.DB, a, "mcp.get_supplier", "suppliers", p.ID, "success", map[string]string{})
			}
			return nil, v, safeError(e)
		})
		mcp.AddTool(server, &mcp.Tool{Name: "get_supplier_price_history", Description: "Consultar observaciones de precio con fuente y fecha, sin tratarlas como cotizaciones oficiales.", OutputSchema: outputSchema("price_page")}, func(ctx context.Context, req *mcp.CallToolRequest, p struct {
			ID         string `json:"id"`
			PartNumber string `json:"part_number,omitempty"`
			Page       int    `json:"page"`
			PerPage    int    `json:"per_page"`
		}) (*mcp.CallToolResult, PricePage, error) {
			a, e := api.ActorFrom(ctx)
			if e != nil {
				return nil, PricePage{}, e
			}
			if p.Page == 0 {
				p.Page = 1
			}
			if p.PerPage == 0 {
				p.PerPage = 25
			}
			v, e := s.PriceHistory(ctx, a, p.ID, p.PartNumber, p.Page, p.PerPage)
			if e == nil {
				e = s.Core.Audit(ctx, s.Core.DB, a, "mcp.get_supplier_price_history", "suppliers", p.ID, "success", map[string]int{"returned": len(v.Data)})
			}
			return nil, v, safeError(e)
		})
		mcp.AddTool(server, &mcp.Tool{Name: "resolve_supplier_exact", Description: "Resolver nombre canónico o RTN contra proveedores activos sin crear ni fusionar.", OutputSchema: outputSchema("match")}, func(ctx context.Context, req *mcp.CallToolRequest, p struct {
			Name string `json:"name"`
			RTN  string `json:"rtn,omitempty"`
		}) (*mcp.CallToolResult, map[string]any, error) {
			a, e := api.ActorFrom(ctx)
			if e != nil {
				return nil, nil, e
			}
			v, e := s.ResolveExact(ctx, a, p.Name, p.RTN)
			if e == nil {
				e = s.Core.Audit(ctx, s.Core.DB, a, "mcp.resolve_supplier_exact", "suppliers", "", "success", map[string]bool{"matched": v != nil})
			}
			return nil, map[string]any{"match": v}, safeError(e)
		})
	}}
}
