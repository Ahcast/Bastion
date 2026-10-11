package invoices

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

func required(fields map[string]json.RawMessage, nullable string, names ...string) error {
	for _, name := range names {
		raw, ok := fields[name]
		if !ok || name != nullable && bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
			return core.Invalid
		}
	}
	return nil
}
func decode(r *http.Request, target any, aggregate bool) error {
	body, e := io.ReadAll(r.Body)
	if e != nil {
		return core.Invalid
	}
	r.Body = io.NopCloser(bytes.NewReader(body))
	if e = api.Decode(r, target); e != nil {
		return e
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(body, &fields) != nil {
		return core.Invalid
	}
	if e = required(fields, "", "currency", "items", "general_discount_type", "general_discount", "exoneration_reference", "additional_expenses"); e != nil {
		return e
	}
	if aggregate {
		if e = required(fields, "credit_days", "customer", "invoice_date", "payment_term_type", "credit_days", "public_notes", "team_id", "version"); e != nil {
			return e
		}
		var customer map[string]json.RawMessage
		if json.Unmarshal(fields["customer"], &customer) != nil {
			return core.Invalid
		}
		if e = required(customer, "", "id", "name", "rtn", "contact", "phone", "email", "address"); e != nil {
			return e
		}
	}
	var items []map[string]json.RawMessage
	if json.Unmarshal(fields["items"], &items) != nil {
		return core.Invalid
	}
	for _, item := range items {
		if e = required(item, "", "description", "unit", "quantity", "unit_price", "unit_cost", "discount_type", "discount", "tax"); e != nil {
			return e
		}
		if raw, ok := item["id"]; ok && bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
			return core.Invalid
		}
	}
	return nil
}
func invoiceFilter(r *http.Request) (Filter, error) {
	q := r.URL.Query()
	f := Filter{Page: 1, PerPage: 25, Search: q.Get("search"), CustomerID: q.Get("customer_id"), Status: q.Get("status")}
	var e error
	if q.Get("page") != "" {
		f.Page, e = strconv.Atoi(q.Get("page"))
		if e != nil {
			return f, core.Invalid
		}
	}
	if q.Get("per_page") != "" {
		f.PerPage, e = strconv.Atoi(q.Get("per_page"))
		if e != nil {
			return f, core.Invalid
		}
	}
	return f, nil
}
func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return registry.Module{ID: "invoices", Label: "Facturas", Version: "0.2.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Routes: func(mux *http.ServeMux) {
		register := func(pattern string, action api.Action) { mux.Handle(pattern, api.Operation(s.Core, logger, action)) }
		register("POST /api/v1/invoices/calculate", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if e := s.gate(ctx, s.Core.DB, a, "facturas.view"); e != nil {
				return nil, e
			}
			var p CalculationInput
			if e := decode(r, &p, false); e != nil {
				return nil, e
			}
			return Calculate(p)
		})
		register("GET /api/v1/invoices", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := invoiceFilter(r)
			if e != nil {
				return nil, e
			}
			return s.List(ctx, a, f)
		})
		register("GET /api/v1/invoices/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			return s.Get(ctx, a, r.PathValue("id"))
		})
		for _, method := range []string{"POST", "PATCH"} {
			path := "/api/v1/invoices"
			if method == "PATCH" {
				path += "/{id}"
			}
			register(method+" "+path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				key := r.Header.Get("Idempotency-Key")
				if key == "" {
					return nil, core.Invalid
				}
				var p Input
				if e := decode(r, &p, true); e != nil {
					return nil, e
				}
				return s.Save(ctx, a, r.PathValue("id"), key, p)
			})
		}
	}}
}
