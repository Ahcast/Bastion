package quotes

import (
	"context"
	"net/http"
	"strconv"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
)

func fxFilter(r *http.Request) (FXFilter, error) {
	f := FXFilter{RateKind: strings.ToLower(strings.TrimSpace(r.URL.Query().Get("rate_kind"))), Source: strings.ToLower(strings.TrimSpace(r.URL.Query().Get("source"))), Page: 1, PerPage: 25}
	var e error
	if v := r.URL.Query().Get("page"); v != "" {
		f.Page, e = strconv.Atoi(v)
		if e != nil {
			return f, core.Invalid
		}
	}
	if v := r.URL.Query().Get("per_page"); v != "" {
		f.PerPage, e = strconv.Atoi(v)
		if e != nil {
			return f, core.Invalid
		}
	}
	return f, nil
}

func (s *Service) registerFX(register func(string, api.Action)) {
	register("GET /api/v1/quotes/fx-observations", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		f, e := fxFilter(r)
		if e != nil {
			return nil, e
		}
		return s.ListFXObservations(ctx, a, f)
	})
	register("GET /api/v1/quotes/fx-observations/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		return s.GetFXObservation(ctx, a, r.PathValue("id"))
	})
	register("POST /api/v1/quotes/fx-observations", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		var p FXObservationInput
		if e := required(r, &p, "rate_kind", "rate", "valid_from", "valid_to", "source", "source_reference", "source_published_at", "evidence_hash", "supersedes_id", "reason"); e != nil {
			return nil, e
		}
		return s.RecordFXObservation(ctx, a, r.Header.Get("Idempotency-Key"), p)
	})
}
