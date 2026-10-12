package api

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"log/slog"
	"mime"
	"net/http"
	"strconv"
	"strings"
)

// ActorFrom only returns a complete authenticated context created by Core middleware.
// A business module must still authorize action/resource/state in its application service.
func ActorFrom(ctx context.Context) (core.Actor, error) {
	x, ok := ctx.Value(contextKey{}).(request)
	if !ok || !x.Session.MFA || x.Actor.UserID == "" {
		return core.Actor{}, core.Unauthorized
	}
	return x.Actor, nil
}

// Download is a bounded authenticated response; domain services authorize before returning it.
type Download struct {
	Data                []byte
	Filename, MediaType string
}

type Action func(context.Context, core.Actor, *http.Request) (any, error)

// Operation allows registered modules to reuse Core's identity, JSON and error contract.
func Operation(s *core.Service, logger *slog.Logger, action Action) http.Handler {
	server := &Server{Service: s, Logger: logger}
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		a, e := ActorFrom(r.Context())
		if e != nil {
			server.error(w, r, e)
			return
		}
		v, e := action(r.Context(), a, r)
		if e != nil {
			if strings.HasPrefix(r.URL.Path, "/api/internal/v1/crawler/") {
				_ = s.Audit(r.Context(), s.DB, a, "crawler.request_rejected", "route", r.Pattern, "rejected", map[string]string{})
			} else if e == core.Forbidden {
				_ = s.Audit(r.Context(), s.DB, a, "api.denied", "route", r.Pattern, "denied", map[string]string{})
			}
			server.error(w, r, e)
			return
		}
		if d, ok := v.(Download); ok {
			if _, _, e := mime.ParseMediaType(d.MediaType); e != nil || len(d.Data) > 32*1024*1024 {
				server.error(w, r, core.Invalid)
				return
			}
			if e := r.Context().Err(); e != nil {
				server.error(w, r, e)
				return
			}
			w.Header().Set("Content-Type", d.MediaType)
			w.Header().Set("Content-Disposition", mime.FormatMediaType("attachment", map[string]string{"filename": d.Filename}))
			w.Header().Set("Cache-Control", "no-store")
			w.Header().Set("X-Content-Type-Options", "nosniff")
			w.Header().Set("Content-Length", strconv.Itoa(len(d.Data)))
			w.WriteHeader(http.StatusOK)
			_, _ = w.Write(d.Data)
			return
		}
		if v == nil {
			v = map[string]bool{"ok": true}
		}
		server.json(w, 200, v)
	})
}
