package api

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"log/slog"
	"net/http"
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
			if e == core.Forbidden {
				_ = s.Audit(r.Context(), s.DB, a, "api.denied", "route", r.Pattern, "denied", map[string]string{})
			}
			server.error(w, r, e)
			return
		}
		if v == nil {
			v = map[string]bool{"ok": true}
		}
		server.json(w, 200, v)
	})
}
