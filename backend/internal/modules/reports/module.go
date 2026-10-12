package reports

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"log/slog"
	"net/http"
	"strconv"
)

func filter(r *http.Request, home bool) (Filter, error) {
	v := r.URL.Query()
	f := Filter{From: v.Get("date_from"), To: v.Get("date_to"), Currency: v.Get("currency"), Status: v.Get("status"), Page: 1, PerPage: 25}
	allowed := map[string]bool{"date_from": true, "date_to": true, "currency": true}
	if !home {
		allowed["status"] = true
		allowed["page"] = true
		allowed["per_page"] = true
	}
	for k, x := range v {
		if !allowed[k] || len(x) != 1 {
			return f, core.Invalid
		}
	}
	var e error
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
func safe(e error) error {
	if e == nil || core.IsClient(e) {
		return e
	}
	return &core.Error{Code: "internal", Message: "No se pudo completar el reporte.", Status: 500}
}
func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return registry.Module{ID: "reports", Label: "Reportes", Version: "0.1.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Routes: func(mux *http.ServeMux) {
		register := func(path string, fn api.Action) {
			mux.Handle(path, api.Operation(s.Core, logger, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				v, e := fn(ctx, a, r)
				return v, safe(e)
			}))
		}
		for _, kind := range []string{"invoices", "orders", "expenses"} {
			if !s.Sources[kind] {
				continue
			}
			register("GET /api/v1/reports/"+kind, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				f, e := filter(r, false)
				if e != nil {
					return nil, e
				}
				return s.Read(ctx, a, kind, f)
			})
		}
		register("GET /api/v1/reports/home", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r, true)
			if e != nil {
				return nil, e
			}
			return s.Home(ctx, a, f)
		})
	}}
}
