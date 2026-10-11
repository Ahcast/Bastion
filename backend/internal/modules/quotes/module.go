package quotes

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

func required(r *http.Request, v any, names ...string) error {
	b, e := io.ReadAll(r.Body)
	if e != nil {
		return core.Invalid
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(b, &fields) != nil {
		return core.Invalid
	}
	for _, n := range names {
		f, ok := fields[n]
		if !ok || (bytes.Equal(bytes.TrimSpace(f), []byte("null")) && !nullableConversionField(v, n)) {
			return core.Invalid
		}
	}
	// Replacing an aggregate must not silently clear omitted nested fields.
	if _, ok := v.(*Input); ok {
		var customer map[string]json.RawMessage
		if json.Unmarshal(fields["customer"], &customer) != nil {
			return core.Invalid
		}
		for _, n := range []string{"id", "name", "rtn", "contact", "phone", "email", "address"} {
			raw, ok := customer[n]
			if !ok || bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
				return core.Invalid
			}
		}
		var items []map[string]json.RawMessage
		if json.Unmarshal(fields["items"], &items) != nil {
			return core.Invalid
		}
		for _, item := range items {
			for _, n := range []string{"description", "unit", "quantity", "unit_price", "unit_cost", "discount_type", "discount", "tax"} {
				raw, ok := item[n]
				if !ok || bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
					return core.Invalid
				}
			}
		}
	}
	r.Body = io.NopCloser(bytes.NewReader(b))
	return api.Decode(r, v)
}

func filter(r *http.Request) (Filter, error) {
	f := Filter{Search: r.URL.Query().Get("search"), Page: 1, PerPage: 25}
	var e error
	for n, d := range map[string]*int{"page": &f.Page, "per_page": &f.PerPage} {
		if v := r.URL.Query().Get(n); v != "" {
			*d, e = strconv.Atoi(v)
			if e != nil {
				return f, core.Invalid
			}
		}
	}
	if v := r.URL.Query().Get("deleted"); v != "" {
		if v != "true" && v != "false" {
			return f, core.Invalid
		}
		f.Deleted = v == "true"
	}
	return f, nil
}

func safe(e error) error {
	if e == nil || core.IsClient(e) {
		return e
	}
	return &core.Error{Code: "internal", Message: "No se pudo completar la operación.", Status: 500}
}

func object(v any) map[string]any {
	b, _ := json.Marshal(v)
	var m map[string]any
	_ = json.Unmarshal(b, &m)
	return m
}

func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return s.ModuleFiles(logger, enabled, nil)
}

func (s *Service) ModuleFiles(logger *slog.Logger, enabled bool, objects core.Objects) registry.Module {
	return registry.Module{ID: "quotes", Label: "Cotizaciones", Version: "0.1.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Routes: func(mux *http.ServeMux) {
		register := func(p string, f api.Action) { mux.Handle(p, api.Operation(s.Core, logger, f)) }
		register("GET /api/v1/quotes", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.List(ctx, a, f)
		})
		register("GET /api/v1/quotes/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			return s.Get(ctx, a, r.PathValue("id"))
		})
		for _, m := range []string{"POST", "PATCH"} {
			path := "/api/v1/quotes"
			if m == "PATCH" {
				path += "/{id}"
			}
			register(m+" "+path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p Input
				if e := required(r, &p, "customer", "reference", "date", "validity_days", "currency", "terms", "private_notes", "author_contact", "exoneration_reference", "general_discount_type", "general_discount", "items", "version"); e != nil {
					return nil, e
				}
				return s.Save(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
		}
		register("POST /api/v1/quotes/calculate", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := s.gate(ctx, s.Core.DB, a, "cotizaciones.view"); e != nil {
				return nil, e
			}
			var p CalculationInput
			if e := required(r, &p, "items", "general_discount_type", "general_discount", "exoneration_reference"); e != nil {
				return nil, e
			}
			return Calculate(p)
		})
		s.registerPurchaseOrderConversion(register)
		s.registerBulk(register)
		s.registerFX(register)
		if s.Approvals != nil {
			register("POST /api/v1/quotes/{id}/emit", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p EmitInput
				if e := required(r, &p, "version", "approval_id"); e != nil {
					return nil, e
				}
				return s.Emit(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
			register("GET /api/v1/quotes/{id}/approvals", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				f, e := filter(r)
				if e != nil {
					return nil, e
				}
				return s.ApprovalList(ctx, a, r.PathValue("id"), f.Page, f.PerPage)
			})
			register("POST /api/v1/quotes/{id}/approvals", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p ApprovalRequest
				if e := required(r, &p, "quote_version", "expires_at", "reason"); e != nil {
					return nil, e
				}
				return s.RequestApproval(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
			register("POST /api/v1/quotes/{id}/approvals/{approval}/decision", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p ApprovalDecision
				if e := required(r, &p, "quote_version", "version", "decision", "reason"); e != nil {
					return nil, e
				}
				return s.DecideApproval(ctx, a, r.PathValue("id"), r.PathValue("approval"), r.Header.Get("Idempotency-Key"), p)
			})
			register("POST /api/v1/quotes/{id}/approvals/{approval}/revoke", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p ApprovalRevoke
				if e := required(r, &p, "quote_version", "version", "reason"); e != nil {
					return nil, e
				}
				return s.RevokeApproval(ctx, a, r.PathValue("id"), r.PathValue("approval"), r.Header.Get("Idempotency-Key"), p)
			})
			register("GET /api/v1/quotes/{id}/email-authorizations", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				f, e := filter(r)
				if e != nil {
					return nil, e
				}
				return s.EmailAuthorizations(ctx, a, r.PathValue("id"), f.Page, f.PerPage)
			})
			register("POST /api/v1/quotes/{id}/email-authorizations", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p EmailAuthorizationRequest
				if e := required(r, &p, "quote_version", "pdf_snapshot_id", "to", "cc", "bcc", "attachment_ids", "subject", "message", "scheduled_at", "expires_at", "reason"); e != nil {
					return nil, e
				}
				return s.RequestEmailAuthorization(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
			register("POST /api/v1/quotes/{id}/email-authorizations/{intent}/decision", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p EmailAuthorizationDecision
				if e := required(r, &p, "version", "decision", "reason"); e != nil {
					return nil, e
				}
				return s.DecideEmailAuthorization(ctx, a, r.PathValue("id"), r.PathValue("intent"), r.Header.Get("Idempotency-Key"), p)
			})
			register("POST /api/v1/quotes/{id}/email-authorizations/{intent}/revoke", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p EmailAuthorizationRevoke
				if e := required(r, &p, "version", "reason"); e != nil {
					return nil, e
				}
				return s.RevokeEmailAuthorizationSafe(ctx, a, r.PathValue("id"), r.PathValue("intent"), r.Header.Get("Idempotency-Key"), p)
			})
			register("POST /api/v1/quotes/{id}/email-authorizations/{intent}/send", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				return s.QueueEmail(ctx, a, r.PathValue("id"), r.PathValue("intent"), r.Header.Get("Idempotency-Key"))
			})
		}
		register("PATCH /api/v1/quotes/{id}/status", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p TransitionInput
			if e := required(r, &p, "status", "version"); e != nil {
				return nil, e
			}
			return s.Transition(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("GET /api/v1/quotes/{id}/versions", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.Versions(ctx, a, r.PathValue("id"), f.Page, f.PerPage)
		})
		for _, op := range []string{"clone", "delete", "restore"} {
			method, path := "POST", "/api/v1/quotes/{id}/"+op
			if op == "delete" {
				method = "DELETE"
				path = "/api/v1/quotes/{id}"
			}
			register(method+" "+path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p struct {
					Version uint64 `json:"version,string"`
				}
				if e := required(r, &p, "version"); e != nil {
					return nil, e
				}
				if op == "clone" {
					return s.Clone(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Version)
				}
				return s.Trash(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Version, op == "restore")
			})
		}
		register("GET /api/v1/quotes/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.Documents(ctx, a, r.PathValue("id"), f.Page, f.PerPage, f.Deleted)
		})
		register("POST /api/v1/quotes/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p struct {
				Document        string `json:"document_id"`
				Version         uint64 `json:"version,string"`
				DocumentVersion uint64 `json:"document_version,string"`
			}
			if e := required(r, &p, "document_id", "version", "document_version"); e != nil {
				return nil, e
			}
			return s.LinkDocument(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Document, p.Version, p.DocumentVersion)
		})
		register("POST /api/v1/quotes/{id}/pdf", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p struct {
				Version uint64 `json:"version,string"`
			}
			if e := required(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.GeneratePDF(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Version)
		})
		register("GET /api/v1/quotes/{id}/pdfs", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.PDFs(ctx, a, r.PathValue("id"), f.Page, f.PerPage)
		})
		register("GET /api/v1/quotes/{id}/pdfs/{snapshot}/download", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			id, e := s.PDFDocument(ctx, a, r.PathValue("id"), r.PathValue("snapshot"))
			if e != nil {
				return nil, e
			}
			return s.Core.DownloadFile(ctx, a, id, 0, objects)
		})

		if s.Processes != nil {
			register("POST /api/v1/processes/{id}/create-quote", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				var p FromProcessInput
				if e := required(r, &p, "date", "version"); e != nil {
					return nil, e
				}
				return s.CreateFromProcess(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
			})
			register("GET /api/v1/processes/{id}/quotes", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				f, e := filter(r)
				if e != nil {
					return nil, e
				}
				return s.Related(ctx, a, r.PathValue("id"), f)
			})
		}
	}, Tools: func(server *mcp.Server) {
		mcp.AddTool(server, &mcp.Tool{Name: "get_quote", Description: "Leer una cotización autorizada, incluidos sus datos internos."}, func(ctx context.Context, req *mcp.CallToolRequest, p struct {
			ID string `json:"id"`
		}) (*mcp.CallToolResult, map[string]any, error) {
			a, e := api.ActorFrom(ctx)
			if e != nil {
				return nil, nil, e
			}
			v, e := s.Get(ctx, a, p.ID)
			if e == nil {
				e = s.Core.Audit(ctx, s.Core.DB, a, "mcp.get_quote", "quotes", p.ID, "success", map[string]string{})
			}
			return nil, object(v), safe(e)
		})
		mcp.AddTool(server, &mcp.Tool{Name: "search_quotes", Description: "Buscar resúmenes autorizados de cotizaciones."}, func(ctx context.Context, req *mcp.CallToolRequest, f Filter) (*mcp.CallToolResult, map[string]any, error) {
			a, e := api.ActorFrom(ctx)
			if e != nil {
				return nil, nil, e
			}
			if f.Page == 0 {
				f.Page = 1
			}
			if f.PerPage == 0 {
				f.PerPage = 25
			}
			v, e := s.List(ctx, a, f)
			if e == nil {
				e = s.Core.Audit(ctx, s.Core.DB, a, "mcp.search_quotes", "quotes", "", "success", map[string]int{"returned": len(v.Data)})
			}
			return nil, object(v), safe(e)
		})
	}}
}
