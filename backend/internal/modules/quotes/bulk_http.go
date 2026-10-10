package quotes

import (
	"context"
	"net/http"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
)

func (s *Service) registerBulk(register func(string, api.Action)) {
	register("POST /api/v1/quotes/bulk/status", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		var p BulkStatusInput
		if e := required(r, &p, "items", "status"); e != nil {
			return nil, e
		}
		return s.BulkStatus(ctx, a, r.Header.Get("Idempotency-Key"), p)
	})
	register("POST /api/v1/quotes/bulk/clone", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		var p BulkCloneInput
		if e := required(r, &p, "items"); e != nil {
			return nil, e
		}
		return s.BulkClone(ctx, a, r.Header.Get("Idempotency-Key"), p)
	})
	register("POST /api/v1/quotes/bulk/export", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
		var p BulkExportInput
		if e := required(r, &p, "items"); e != nil {
			return nil, e
		}
		return s.BulkExport(ctx, a, r.Header.Get("Idempotency-Key"), p)
	})
}
