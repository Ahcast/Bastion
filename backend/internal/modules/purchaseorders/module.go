package purchaseorders

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"strconv"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
)

func orderPage(r *http.Request) (int, int, error) {
	page, perPage := 1, 25
	var e error
	q := r.URL.Query()
	if q.Get("page") != "" {
		page, e = strconv.Atoi(q.Get("page"))
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if q.Get("per_page") != "" {
		perPage, e = strconv.Atoi(q.Get("per_page"))
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if page < 1 || page > 100000 || perPage < 1 || perPage > 100 {
		return 0, 0, core.Invalid
	}
	return page, perPage, nil
}

func orderFilter(r *http.Request) (Filter, error) {
	page, perPage, e := orderPage(r)
	if e != nil {
		return Filter{}, e
	}
	q := r.URL.Query()
	return Filter{
		Search:     q.Get("search"),
		CustomerID: q.Get("customer_id"),
		Status:     q.Get("status"),
		TotalMin:   q.Get("total_min"),
		TotalMax:   q.Get("total_max"),
		Page:       page,
		PerPage:    perPage,
	}, nil
}

func strictRequired(r *http.Request, target any, nullable map[string]bool, names ...string) (map[string]json.RawMessage, error) {
	body, e := io.ReadAll(r.Body)
	if e != nil {
		return nil, core.Invalid
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(body, &fields) != nil {
		return nil, core.Invalid
	}
	for _, name := range names {
		raw, ok := fields[name]
		if !ok || (!nullable[name] && bytes.Equal(bytes.TrimSpace(raw), []byte("null"))) {
			return nil, core.Invalid
		}
	}
	r.Body = io.NopCloser(bytes.NewReader(body))
	if e := api.Decode(r, target); e != nil {
		return nil, e
	}
	return fields, nil
}

func requireNested(fields map[string]json.RawMessage, aggregate bool) error {
	if aggregate {
		var customer map[string]json.RawMessage
		if json.Unmarshal(fields["customer"], &customer) != nil {
			return core.Invalid
		}
		for _, name := range []string{"id", "name", "rtn", "contact", "phone", "email", "address"} {
			raw, ok := customer[name]
			if !ok || bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
				return core.Invalid
			}
		}
	}
	var items []map[string]json.RawMessage
	if json.Unmarshal(fields["items"], &items) != nil {
		return core.Invalid
	}
	financial := []string{"description", "unit", "quantity", "unit_sale_price", "unit_cost", "discount_type", "discount", "tax_type"}
	persistent := []string{"supplier_id", "supplier_name_snapshot", "tracking", "comments"}
	for _, item := range items {
		for _, name := range financial {
			raw, ok := item[name]
			if !ok || bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
				return core.Invalid
			}
		}
		if aggregate {
			for _, name := range persistent {
				raw, ok := item[name]
				if !ok || bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
					return core.Invalid
				}
			}
		}
	}
	return nil
}

func decodeOrderInput(r *http.Request, p *Input) error {
	fields, e := strictRequired(r, p, map[string]bool{"required_delivery_date": true},
		"order_number", "customer", "destination", "order_date", "required_delivery_date", "currency", "comments",
		"exoneration_reference", "general_discount_type", "general_discount", "additional_expenses", "team_id", "version", "items",
	)
	if e != nil {
		return e
	}
	return requireNested(fields, true)
}

func decodeCalculation(r *http.Request, p *CalculationInput) error {
	fields, e := strictRequired(r, p, map[string]bool{}, "items", "general_discount_type", "general_discount", "exoneration_reference", "additional_expenses")
	if e != nil {
		return e
	}
	return requireNested(fields, false)
}

func requireIdempotency(r *http.Request) (string, error) {
	key := r.Header.Get("Idempotency-Key")
	if strings.TrimSpace(key) == "" {
		return "", core.Invalid
	}
	return key, nil
}

func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return registry.Module{
		ID:           "purchase-orders",
		Label:        "Órdenes de Compra",
		Version:      "0.4.0",
		Enabled:      enabled,
		Dependencies: []string{"core"},
		Permissions:  Permissions,
		Routes: func(mux *http.ServeMux) {
			register := func(pattern string, action api.Action) {
				mux.Handle(pattern, api.Operation(s.Core, logger, action))
			}
			register("POST /api/v1/purchase-orders/calculate", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				if e := s.gate(ctx, s.Core.DB, a, "ordenes.view"); e != nil {
					return nil, e
				}
				var p CalculationInput
				if e := decodeCalculation(r, &p); e != nil {
					return nil, e
				}
				return Calculate(p)
			})
			register("GET /api/v1/purchase-orders", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				f, e := orderFilter(r)
				if e != nil {
					return nil, e
				}
				return s.List(ctx, a, f)
			})
			register("GET /api/v1/purchase-orders/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				return s.Get(ctx, a, r.PathValue("id"))
			})
			register("GET /api/v1/purchase-orders/{id}/status-history", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				page, perPage, e := orderPage(r)
				if e != nil {
					return nil, e
				}
				return s.StatusHistory(ctx, a, r.PathValue("id"), page, perPage)
			})
			register("GET /api/v1/purchase-orders/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				page, limit, e := orderPage(r)
				if e != nil {
					return nil, e
				}
				deleted := r.URL.Query().Get("deleted")
				if deleted != "" && deleted != "false" && deleted != "true" {
					return nil, core.Invalid
				}
				return s.Documents(ctx, a, r.PathValue("id"), r.URL.Query().Get("item_id"), page, limit, deleted == "true")
			})
			register("POST /api/v1/purchase-orders/{id}/documents", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				key, e := requireIdempotency(r)
				if e != nil {
					return nil, e
				}
				var p DocumentInput
				if _, e = strictRequired(r, &p, map[string]bool{"item_id": true}, "document_id", "document_version", "version", "item_id"); e != nil {
					return nil, e
				}
				return s.LinkDocument(ctx, a, r.PathValue("id"), key, p)
			})
			transition := func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				key, e := requireIdempotency(r)
				if e != nil {
					return nil, e
				}
				var p TransitionInput
				if _, e = strictRequired(r, &p, nil, "to_status", "version"); e != nil {
					return nil, e
				}
				position := 0
				if value := r.PathValue("position"); value != "" {
					position, e = strconv.Atoi(value)
					if e != nil || position < 1 || position > 200 {
						return nil, core.Invalid
					}
				}
				return s.Transition(ctx, a, r.PathValue("id"), key, position, p)
			}
			register("POST /api/v1/purchase-orders/{id}/transition", transition)
			register("POST /api/v1/purchase-orders/{id}/items/{position}/transition", transition)
			for _, method := range []string{"POST", "PATCH"} {
				path := "/api/v1/purchase-orders"
				if method == "PATCH" {
					path += "/{id}"
				}
				register(method+" "+path, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
					key, e := requireIdempotency(r)
					if e != nil {
						return nil, e
					}
					var p Input
					if e = decodeOrderInput(r, &p); e != nil {
						return nil, e
					}
					return s.Save(ctx, a, r.PathValue("id"), key, p)
				})
			}
		},
	}
}
