package api

import (
	"context"
	"encoding/json"
	"errors"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/go-sql-driver/mysql"
	"io"
	"log/slog"
	"mime"
	"net"
	"net/http"
	"strconv"
	"strings"
	"time"
)

type Server struct {
	Service  *core.Service
	Registry *registry.Registry
	Objects  core.Objects
	Logger   *slog.Logger
	mux      *http.ServeMux
}
type request struct {
	Actor   core.Actor
	Session core.Session
}
type contextKey struct{}
type endpoint func(http.ResponseWriter, *http.Request, request) (any, error)

func New(s *core.Service, r *registry.Registry, objects core.Objects, logger *slog.Logger) http.Handler {
	v := &Server{s, r, objects, logger, http.NewServeMux()}
	v.routes()
	r.Mount(v.mux)
	return v.middleware(v.mux)
}
func (s *Server) CookieName() string {
	if s.Service.Config.Env == "production" {
		return "__Host-bastion_session"
	}
	return "bastion_dev_session"
}
func (s *Server) cookie(w http.ResponseWriter, token string) {
	maxAge := int(s.Service.Config.Absolute.Seconds())
	if token == "" {
		maxAge = -1
	}
	http.SetCookie(w, &http.Cookie{Name: s.CookieName(), Value: token, Path: "/", Secure: s.Service.Config.Env == "production", HttpOnly: true, SameSite: http.SameSiteLaxMode, MaxAge: maxAge})
}
func (s *Server) json(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}
func (s *Server) error(w http.ResponseWriter, r *http.Request, e error) {
	var ce *core.Error
	if !errors.As(e, &ce) {
		var me *mysql.MySQLError
		if errors.As(e, &me) && (me.Number == 1062 || me.Number == 1452) {
			ce = core.Conflict
		} else {
			ce = &core.Error{Code: "internal", Message: "No se pudo completar la operación.", Status: 500}
			s.Logger.Error("request failed", "request_id", w.Header().Get("X-Request-ID"), "method", r.Method, "path", r.URL.Path)
		}
	}
	s.json(w, ce.Status, map[string]any{"code": ce.Code, "message": ce.Message, "request_id": w.Header().Get("X-Request-ID")})
}
func Decode(r *http.Request, v any) error {
	media, _, err := mime.ParseMediaType(r.Header.Get("Content-Type"))
	if err != nil || media != "application/json" {
		return core.Invalid
	}
	d := json.NewDecoder(r.Body)
	d.DisallowUnknownFields()
	if e := d.Decode(v); e != nil {
		return core.Invalid
	}
	var extra any
	if e := d.Decode(&extra); e != io.EOF {
		return core.Invalid
	}
	return nil
}
func (s *Server) handle(pattern string, authenticated bool, f endpoint) {
	s.mux.HandleFunc(pattern, func(w http.ResponseWriter, r *http.Request) {
		x, _ := r.Context().Value(contextKey{}).(request)
		if authenticated && (!x.Session.MFA || x.Actor.UserID == "") {
			s.error(w, r, core.Unauthorized)
			return
		}
		v, e := f(w, r, x)
		if e != nil {
			if errors.Is(e, core.Forbidden) {
				_ = s.Service.Audit(r.Context(), s.Service.DB, x.Actor, "api.denied", "route", r.Pattern, "denied", map[string]string{})
			}
			s.error(w, r, e)
			return
		}
		if v == nil {
			v = map[string]bool{"ok": true}
		}
		s.json(w, 200, v)
	})
}
func (s *Server) middleware(next http.Handler) http.Handler {
	slots := make(chan struct{}, 32)
	crawlerSlots := make(chan struct{}, 4)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		budget := 25 * time.Second
		if strings.HasPrefix(r.URL.Path, "/api/internal/v1/crawler/") {
			budget = 75 * time.Second
		}
		ctx, cancel := context.WithTimeout(r.Context(), budget)
		defer cancel()
		r = r.WithContext(ctx)
		id := security.ID()
		w.Header().Set("X-Request-ID", id)
		select {
		case slots <- struct{}{}:
			defer func() { <-slots }()
		default:
			s.error(w, r, &core.Error{Code: "rate_limited", Message: "El servidor está ocupado. Intente más tarde.", Status: 429})
			return
		}
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Referrer-Policy", "no-referrer")
		w.Header().Set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'")
		if s.Service.Config.Env == "production" {
			w.Header().Set("Strict-Transport-Security", "max-age=31536000")
		}
		defer func() {
			if recover() != nil {
				s.Logger.Error("panic recovered", "request_id", id)
				s.error(w, r, errors.New("panic"))
			}
		}()
		origin := r.Header.Get("Origin")
		if origin != "" && origin != s.Service.Config.Origin {
			s.error(w, r, core.Forbidden)
			return
		}
		if origin != "" {
			w.Header().Set("Access-Control-Allow-Origin", origin)
			w.Header().Set("Vary", "Origin")
			w.Header().Set("Access-Control-Allow-Credentials", "true")
			w.Header().Set("Access-Control-Expose-Headers", "X-Request-ID")
		}
		if r.Method == "OPTIONS" {
			if origin == "" {
				s.error(w, r, core.Forbidden)
				return
			}
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type, X-CSRF-Token, Idempotency-Key, X-Bastion-Activity, MCP-Protocol-Version, Authorization")
			w.WriteHeader(204)
			return
		}
		if r.URL.Path == "/healthz" {
			s.json(w, 200, map[string]string{"status": "ok"})
			return
		}
		if r.URL.Path == "/readyz" {
			ctx, cancel := context.WithTimeout(r.Context(), 2*time.Second)
			defer cancel()
			if e := database.Ready(ctx, s.Service.DB); e != nil {
				s.json(w, 503, map[string]string{"status": "unavailable"})
				return
			}
			s.json(w, 200, map[string]string{"status": "ready"})
			return
		}
		bodyLimit := int64(1024 * 1024)
		if strings.HasPrefix(r.URL.Path, "/api/internal/v1/crawler/") {
			select {
			case crawlerSlots <- struct{}{}:
				defer func() { <-crawlerSlots }()
			default:
				s.error(w, r, &core.Error{Code: "rate_limited", Message: "El importador está ocupado. Reintente más tarde.", Status: 429})
				return
			}
			bodyLimit = 24 * 1024 * 1024
		}
		r.Body = http.MaxBytesReader(w, r.Body, bodyLimit)
		ip, _, _ := net.SplitHostPort(r.RemoteAddr)
		if ip == "" {
			ip = r.RemoteAddr
		}
		if e := s.Service.Limit(r.Context(), "api-ip:"+ip, 3000); e != nil {
			s.error(w, r, e)
			return
		}
		if strings.HasPrefix(r.URL.Path, "/api/internal/v1/crawler/") {
			header := r.Header.Get("Authorization")
			if !strings.HasPrefix(header, "Bearer ") {
				s.error(w, r, core.Unauthorized)
				return
			}
			a, e := s.Service.AuthenticateService(r.Context(), strings.TrimPrefix(header, "Bearer "))
			if e != nil {
				s.error(w, r, e)
				return
			}
			if e := http.NewResponseController(w).SetWriteDeadline(time.Now().Add(85 * time.Second)); e != nil {
				s.error(w, r, e)
				return
			}
			a.Source = "crawler"
			a.RequestID = id
			a.CorrelationID = id
			next.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), contextKey{}, request{Actor: a, Session: core.Session{MFA: true}})))
			return
		}
		if r.URL.Path == "/mcp" {
			s.serveMCP(w, r, id)
			return
		}
		correlation := r.Header.Get("X-Correlation-ID")
		if len(correlation) != 32 || strings.IndexFunc(correlation, func(c rune) bool { return !strings.ContainsRune("0123456789abcdef", c) }) >= 0 {
			correlation = id
		}
		x := request{Actor: core.Actor{Type: "user", Source: "web", RequestID: id, CorrelationID: correlation}}
		token := ""
		if c, e := r.Cookie(s.CookieName()); e == nil {
			token = c.Value
		}
		session, e := s.Service.Session(r.Context(), token)
		if r.Method == "GET" && r.URL.Path == "/api/v1/auth/csrf" && e == core.Unauthorized {
			token, csrf, e := s.Service.NewSession(r.Context(), "", false)
			if e != nil {
				s.error(w, r, e)
				return
			}
			s.cookie(w, token)
			s.json(w, 200, map[string]string{"csrf_token": csrf})
			return
		}
		if e != nil {
			s.error(w, r, e)
			return
		}
		x.Session = session
		x.Actor.ID = session.UserID
		x.Actor.UserID = session.UserID
		x.Actor.SessionHash = session.Hash
		if r.Method != "GET" && r.Method != "HEAD" {
			if origin != s.Service.Config.Origin || !security.Equal(security.Hash(r.Header.Get("X-CSRF-Token")), session.CSRFHash) {
				s.error(w, r, core.Forbidden)
				return
			}
		}
		// Background polling never refreshes idle timeout. The frontend marks requests only following human input.
		if r.Header.Get("X-Bastion-Activity") == "true" {
			if e = s.Service.Touch(r.Context(), session); e != nil {
				s.error(w, r, e)
				return
			}
		}
		next.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), contextKey{}, x)))
	})
}
func page(r *http.Request) (int, int, error) {
	limit := 25
	p := 1
	var e error
	if x := r.URL.Query().Get("per_page"); x != "" {
		limit, e = strconv.Atoi(x)
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if x := r.URL.Query().Get("page"); x != "" {
		p, e = strconv.Atoi(x)
		if e != nil {
			return 0, 0, core.Invalid
		}
	}
	if limit < 1 || limit > 100 || p < 1 || p > 100000 {
		return 0, 0, core.Invalid
	}
	return limit, (p - 1) * limit, nil
}
func (s *Server) routes() {
	s.handle("GET /api/v1/auth/preferences", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Preferences(r.Context(), x.Actor)
	})
	s.handle("PATCH /api/v1/auth/preferences", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p core.Preferences
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.SavePreferences(r.Context(), x.Actor, p)
	})
	s.handle("POST /api/v1/auth/recovery-codes", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		codes, e := s.Service.RecoveryCodes(r.Context(), x.Actor)
		return map[string]any{"recovery_codes": codes}, e
	})
	s.handle("POST /api/v1/auth/password", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Password string `json:"password"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		e := s.Service.ChangePassword(r.Context(), x.Actor, p.Password)
		if e == nil {
			s.cookie(w, "")
		}
		return nil, e
	})

	s.handle("GET /api/v1/auth/csrf", false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		token, e := s.Service.RotateCSRF(r.Context(), x.Session)
		return map[string]string{"csrf_token": token}, e
	})
	s.handle("POST /api/v1/auth/login", false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Email    string `json:"email"`
			Password string `json:"password"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		ip, _, _ := net.SplitHostPort(r.RemoteAddr)
		if e := s.Service.Limit(r.Context(), "login-ip:"+ip, 60); e != nil {
			return nil, e
		}
		token, csrf, e := s.Service.Login(r.Context(), x.Session, x.Actor, p.Email, p.Password)
		if e != nil {
			return nil, e
		}
		s.cookie(w, token)
		return map[string]any{"csrf_token": csrf, "mfa_required": true}, nil
	})
	s.handle("GET /api/v1/auth/status", false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		if x.Actor.UserID == "" {
			return nil, core.Unauthorized
		}
		u, e := s.Service.Profile(r.Context(), x.Actor.UserID)
		return map[string]any{"mfa_complete": x.Session.MFA, "mfa_enabled": u.MFA, "email": u.Email}, e
	})
	s.handle("POST /api/v1/auth/mfa/enroll", false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		uri, e := s.Service.Enroll(r.Context(), x.Session)
		return map[string]string{"otpauth_uri": uri}, e
	})
	for _, route := range []string{"confirm", "challenge"} {
		enroll := route == "confirm"
		s.handle("POST /api/v1/auth/mfa/"+route, false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
			var p struct {
				Code string `json:"code"`
			}
			if e := Decode(r, &p); e != nil {
				return nil, e
			}
			token, csrf, codes, e := s.Service.CompleteMFA(r.Context(), x.Session, x.Actor, p.Code, enroll)
			if e != nil {
				return nil, e
			}
			s.cookie(w, token)
			return map[string]any{"csrf_token": csrf, "recovery_codes": codes}, nil
		})
	}
	s.handle("POST /api/v1/auth/reauth", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Password string `json:"password"`
			Code     string `json:"code"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.Reauthenticate(r.Context(), x.Session, x.Actor, p.Password, p.Code)
	})
	s.handle("POST /api/v1/auth/logout", false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		e := s.Service.Logout(r.Context(), x.Session, x.Actor)
		if e == nil {
			s.cookie(w, "")
		}
		return nil, e
	})
	s.handle("GET /api/v1/auth/me", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		u, e := s.Service.Profile(r.Context(), x.Actor.UserID)
		if e != nil {
			return nil, e
		}
		g, e := s.Service.Grants(r.Context(), s.Service.DB, x.Actor.UserID)
		modules := []string{}
		for _, m := range s.Registry.List() {
			if m.Enabled {
				modules = append(modules, m.ID)
			}
		}
		return map[string]any{"user": u, "grants": g, "enabled_modules": modules}, e
	})
	s.handle("POST /api/v1/auth/accept", false, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Token    string `json:"token"`
			Name     string `json:"name"`
			Password string `json:"password"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.AcceptIdentityToken(r.Context(), p.Token, p.Name, p.Password)
	})
	s.handle("GET /api/v1/users", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		l, o, e := page(r)
		if e != nil {
			return nil, e
		}
		return s.Service.Users(r.Context(), x.Actor, l, o)
	})
	s.handle("PATCH /api/v1/users/{id}", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p core.UserChange
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.UpdateUser(r.Context(), x.Actor, r.PathValue("id"), p)
	})
	s.handle("POST /api/v1/users/{id}/reset-mfa", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Reference string `json:"verification_reference"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.ResetMFA(r.Context(), x.Actor, r.PathValue("id"), p.Reference)
	})
	s.handle("DELETE /api/v1/users/{id}/sessions", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return nil, s.Service.RevokeSessions(r.Context(), x.Actor, r.PathValue("id"))
	})
	s.handle("POST /api/v1/invitations", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Email string `json:"email"`
			Kind  string `json:"kind"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return s.Service.CreateIdentityToken(r.Context(), x.Actor, p.Kind, p.Email)
	})
	s.handle("GET /api/v1/roles", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Roles(r.Context(), x.Actor)
	})
	s.handle("POST /api/v1/roles", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p core.Role
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return s.Service.SaveRole(r.Context(), x.Actor, "", p)
	})
	s.handle("PATCH /api/v1/roles/{id}", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p core.Role
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return s.Service.SaveRole(r.Context(), x.Actor, r.PathValue("id"), p)
	})
	s.handle("GET /api/v1/teams", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Teams(r.Context(), x.Actor)
	})
	s.handle("POST /api/v1/teams", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p core.Team
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return s.Service.SaveTeam(r.Context(), x.Actor, "", p)
	})
	s.handle("PATCH /api/v1/teams/{id}", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p core.Team
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return s.Service.SaveTeam(r.Context(), x.Actor, r.PathValue("id"), p)
	})
	s.handle("GET /api/v1/settings", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Settings(r.Context(), x.Actor)
	})
	s.handle("PATCH /api/v1/settings/{name}", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Value   json.RawMessage `json:"value"`
			Secret  string          `json:"secret"`
			Version uint64          `json:"version,string"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.SaveSetting(r.Context(), x.Actor, r.PathValue("name"), core.Setting{Value: p.Value, Version: p.Version}, p.Secret)
	})
	s.handle("GET /api/v1/audit", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		l, o, e := page(r)
		if e != nil {
			return nil, e
		}
		return s.Service.AuditList(r.Context(), x.Actor, l, o)
	})
	s.handle("GET /api/v1/services", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Services(r.Context(), x.Actor)
	})
	s.handle("POST /api/v1/services", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Name    string    `json:"name"`
			Scopes  []string  `json:"scopes"`
			Expires time.Time `json:"expires_at"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return s.Service.CreateService(r.Context(), x.Actor, p.Name, p.Scopes, p.Expires)
	})
	s.handle("DELETE /api/v1/services/{id}", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return nil, s.Service.RevokeService(r.Context(), x.Actor, r.PathValue("id"))
	})
	s.handle("GET /api/v1/modules", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Modules(r.Context(), x.Actor)
	})
	s.handle("GET /api/v1/dashboard", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		u, e := s.Service.Profile(r.Context(), x.Actor.UserID)
		return map[string]any{"name": u.Name, "modules": s.Registry.List(), "integrations": map[string]bool{"mcp": s.Service.IntegrationEnabled(r.Context(), "mcp"), "files": s.Service.IntegrationEnabled(r.Context(), "files"), "email": s.Service.IntegrationEnabled(r.Context(), "email")}}, e
	})
	s.handle("GET /api/v1/jobs", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Jobs(r.Context(), x.Actor)
	})
	s.handle("POST /api/v1/jobs/{id}/retry", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return nil, s.Service.RetryJob(r.Context(), x.Actor, r.PathValue("id"))
	})
	s.handle("GET /api/v1/notifications", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Notifications(r.Context(), x.Actor)
	})
	s.handle("PATCH /api/v1/notifications/{id}/read", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return nil, s.Service.ReadNotification(r.Context(), x.Actor, r.PathValue("id"))
	})
	s.handle("GET /api/v1/emails", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.Emails(r.Context(), x.Actor)
	})
	s.handle("POST /api/v1/emails/test", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return s.Service.QueueTestEmail(r.Context(), x.Actor, r.Header.Get("Idempotency-Key"))
	})
	s.handle("POST /api/v1/emails/{id}/reconcile", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Status    string `json:"status"`
			Reference string `json:"evidence_reference"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.ReconcileEmail(r.Context(), x.Actor, r.PathValue("id"), p.Status, p.Reference)
	})
	for _, restore := range []bool{false, true} {
		pattern := "DELETE /api/v1/documents/{id}"
		if restore {
			pattern = "POST /api/v1/documents/{id}/restore"
		}
		s.handle(pattern, true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
			var p struct {
				Version uint64 `json:"version,string"`
			}
			if e := Decode(r, &p); e != nil {
				return nil, e
			}
			return s.Service.TrashFile(r.Context(), x.Actor, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p.Version, restore)
		})
	}

	s.handle("GET /api/v1/documents", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		l, o, e := page(r)
		if e != nil {
			return nil, e
		}
		return s.Service.Documents(r.Context(), x.Actor, l, o)
	})
	for _, pattern := range []string{"POST /api/v1/documents", "POST /api/v1/documents/{id}/versions"} {
		s.handle(pattern, true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
			var p core.FileRequest
			if e := Decode(r, &p); e != nil {
				return nil, e
			}
			return s.Service.CreateFile(r.Context(), x.Actor, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p, s.Objects)
		})
	}
	s.handle("POST /api/v1/documents/{id}/finalize", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		return nil, s.Service.FinalizeFile(r.Context(), x.Actor, r.PathValue("id"), s.Objects)
	})
	s.handle("POST /api/v1/documents/{id}/scan", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		var p struct {
			Number    int    `json:"number"`
			Clean     bool   `json:"clean"`
			Reference string `json:"scan_reference"`
		}
		if e := Decode(r, &p); e != nil {
			return nil, e
		}
		return nil, s.Service.ScanFile(r.Context(), x.Actor, r.PathValue("id"), p.Number, p.Clean, p.Reference)
	})
	s.handle("GET /api/v1/documents/{id}/download", true, func(w http.ResponseWriter, r *http.Request, x request) (any, error) {
		n := 0
		if q := r.URL.Query().Get("number"); q != "" {
			var e error
			n, e = strconv.Atoi(q)
			if e != nil || n < 1 {
				return nil, core.Invalid
			}
		}
		return s.Service.DownloadFile(r.Context(), x.Actor, r.PathValue("id"), n, s.Objects)
	})
	s.mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) { s.error(w, r, core.Missing) })
}
func (s *Server) Modules(ctx context.Context, a core.Actor) (any, error) {
	if e := s.Service.Authorize(ctx, s.Service.DB, a, "core.modules.read", "", "", false); e != nil {
		return nil, e
	}
	return map[string]any{"data": s.Registry.List()}, nil
}
