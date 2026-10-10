package processes

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

func (s *Service) Module(logger *slog.Logger, enabled bool) registry.Module {
	return registry.Module{ID: "processes", Label: "Procesos", Version: "0.1.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Tools: s.tools, Routes: func(m *http.ServeMux) {
		add := func(pattern string, f api.Action) { m.Handle(pattern, api.Operation(s.Core, logger, f)) }
		add("GET /api/v1/processes/status", func(c context.Context, a core.Actor, r *http.Request) (any, error) { return s.Status(c, a) })
		add("POST /api/v1/processes/export", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.Export(c, a, f)
		})
		add("GET /api/v1/processes", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.List(c, a, f)
		})
		add("GET /api/v1/processes/{id}", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			return s.Get(c, a, r.PathValue("id"))
		})
		add("GET /api/v1/processes/{id}/versions", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			f, e := filter(r)
			if e != nil {
				return nil, e
			}
			return s.Versions(c, a, r.PathValue("id"), f.Page, f.PerPage)
		})
		for _, kind := range []string{"mark", "note"} {
			path := kind
			if kind == "mark" {
				path = "interest"
			}
			add("PATCH /api/v1/processes/{id}/"+path, func(c context.Context, a core.Actor, r *http.Request) (any, error) {
				var p PersonalInput
				if e := decodePersonal(r, &p); e != nil {
					return nil, e
				}
				return s.Personal(c, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), kind, p)
			})
		}
		add("POST /api/internal/v1/crawler/runs", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			var p RunInput
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.CreateRun(c, a, r.Header.Get("Idempotency-Key"), p)
		})
		add("POST /api/internal/v1/crawler/runs/{run_id}/processes", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			var p Batch
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.Import(c, a, r.PathValue("run_id"), r.Header.Get("Idempotency-Key"), p)
		})
		add("POST /api/internal/v1/crawler/runs/{run_id}/complete", func(c context.Context, a core.Actor, r *http.Request) (any, error) {
			var p Complete
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.Complete(c, a, r.PathValue("run_id"), r.Header.Get("Idempotency-Key"), p)
		})
	}}
}

func filter(r *http.Request) (Filter, error) {
	v := r.URL.Query()
	f := Filter{Search: v.Get("search"), Entity: v.Get("entity"), Direction: v.Get("direction"), DateFrom: v.Get("date_from"), DateTo: v.Get("date_to"), Page: 1, PerPage: 25}
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
	for _, x := range []struct {
		name string
		p    **bool
	}{{"interesting", &f.Interesting}, {"source_active", &f.Active}, {"current", &f.Current}} {
		if v.Has(x.name) {
			if v.Get(x.name) != "true" && v.Get(x.name) != "false" {
				return f, core.Invalid
			}
			b := v.Get(x.name) == "true"
			*x.p = &b
		}
	}
	if v.Has("hide_municipalities") {
		if v.Get("hide_municipalities") != "true" && v.Get("hide_municipalities") != "false" {
			return f, core.Invalid
		}
		f.HideMunicipalities = v.Get("hide_municipalities") == "true"
	}
	return f, nil
}

func decodePersonal(r *http.Request, p *PersonalInput) error {
	b, e := io.ReadAll(r.Body)
	if e != nil {
		return core.Invalid
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(b, &fields) != nil {
		return core.Invalid
	}
	v, ok := fields["version"]
	if !ok || bytes.Equal(bytes.TrimSpace(v), []byte("null")) {
		return core.Invalid
	}
	r.Body = io.NopCloser(bytes.NewReader(b))
	return api.Decode(r, p)
}
