package core

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"strconv"
	"strings"
	"time"
)

type Error struct {
	Code    string
	Message string
	Status  int
}

func (e *Error) Error() string { return e.Code }

var Unauthorized = &Error{"unauthenticated", "La sesión no es válida.", 401}
var Forbidden = &Error{"forbidden", "No tiene permiso para esta operación.", 403}
var Conflict = &Error{"conflict", "El registro cambió o la solicitud ya fue utilizada.", 409}
var Invalid = &Error{"validation", "Revise los datos ingresados.", 422}
var Missing = &Error{"not_found", "El registro no está disponible.", 404}
var Disabled = &Error{"integration_disabled", "La integración no está habilitada.", 503}

type Actor struct {
	Type, ID, UserID, SessionHash, Source, RequestID, CorrelationID string
	Scopes                                                          []string
	ServiceID                                                       string
}

func (a Actor) Key() string { return a.Type + ":" + a.ID }

type Session struct {
	Hash, CSRFHash, UserID string
	MFA                    bool
	Created, Last, Expires time.Time
	Reauthenticated        sql.NullTime
}
type User struct {
	Roles   []string `json:"roles"`
	ID      string   `json:"id"`
	Email   string   `json:"email"`
	Name    string   `json:"name"`
	Active  bool     `json:"active"`
	MFA     bool     `json:"mfa_enabled"`
	Version uint64   `json:"version,string"`
}
type Grant struct {
	Permission string `json:"permission"`
	Scope      string `json:"scope"`
}
type Service struct {
	DB                      *sql.DB
	Config                  config.Config
	DummyHash               string
	ModulePermissions       []string
	ServiceReadPermissions  []string
	ServiceWritePermissions []string
	DocumentPolicies        map[string]DocumentPolicy
	DocumentBindingPolicies map[string]DocumentBindingPolicy
	DocumentVersionPolicies map[string]DocumentVersionPolicy
	DocumentTrashPolicies   map[string]DocumentTrashPolicy
}

func New(db *sql.DB, c config.Config) *Service {
	return &Service{DB: db, Config: c, DummyHash: security.Password(security.Token()), DocumentPolicies: map[string]DocumentPolicy{}, DocumentBindingPolicies: map[string]DocumentBindingPolicy{}, DocumentVersionPolicies: map[string]DocumentVersionPolicy{}, DocumentTrashPolicies: map[string]DocumentTrashPolicy{}}
}

// RegisterPermissions registers human/user permissions only. Service identities
// do not inherit module read permissions implicitly; service reads must be
// explicitly reviewed and registered through RegisterServiceReads.
func (s *Service) RegisterPermissions(permissions, _ []string) {
	s.ModulePermissions = append(s.ModulePermissions, permissions...)
}

// RegisterServiceReads permits only explicitly reviewed machine-readable
// surfaces. Do not use this for ordinary UI/API read permissions.
func (s *Service) RegisterServiceReads(permissions []string) {
	s.ServiceReadPermissions = append(s.ServiceReadPermissions, permissions...)
}

// RegisterServiceWrites permits only explicitly composed technical ingestion scopes.
func (s *Service) RegisterServiceWrites(permissions []string) {
	s.ServiceWritePermissions = append(s.ServiceWritePermissions, permissions...)
}
func (s *Service) PermissionCatalog() []string {
	return append(append([]string{}, Permissions...), s.ModulePermissions...)
}

// servicePermissionAllowed is the service-identity allowlist. Core document
// reads are intentionally absent: MCP/service actors must never mint private
// file download URLs. Business module reads are deny-by-default until a
// purpose-built reduced DTO has been reviewed and registered explicitly.
func (s *Service) servicePermissionAllowed(permission string) bool {
	if permission == "core.modules.read" || permission == "core.jobs.read" {
		return true
	}
	return contains(s.ServiceReadPermissions, permission) || contains(s.ServiceWritePermissions, permission)
}

type Querier interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

var Permissions = []string{"core.users.read", "core.users.manage", "core.roles.read", "core.roles.manage", "core.teams.read", "core.teams.manage", "core.settings.read", "core.settings.manage", "core.audit.read", "core.services.manage", "core.modules.read", "core.jobs.read", "core.jobs.retry", "core.documents.read", "core.documents.write", "core.documents.scan", "core.email.test", "core.email.read", "core.notifications.read"}

func ID(v string) error {
	n, e := strconv.ParseUint(v, 10, 64)
	if e != nil || n == 0 {
		return Invalid
	}
	return nil
}
func (s *Service) Transact(ctx context.Context, f func(*sql.Tx) error) error {
	tx, e := s.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if e != nil {
		return e
	}
	defer tx.Rollback()
	if e = f(tx); e != nil {
		return e
	}
	return tx.Commit()
}
func (s *Service) Audit(ctx context.Context, q Querier, a Actor, action, entity, id, result string, details any) error {
	d, e := json.Marshal(details)
	if e != nil {
		return e
	}
	_, e = q.ExecContext(ctx, `INSERT INTO audit_events(actor_type,actor_id,requested_by,action,entity_type,entity_id,source,result,request_id,correlation_id,details,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6))`, a.Type, a.ID, nullable(a.UserID), action, entity, id, a.Source, result, a.RequestID, a.CorrelationID, string(d))
	return e
}
func nullable(s string) any {
	if s == "" {
		return nil
	}
	return s
}
func (s *Service) Authorize(ctx context.Context, q Querier, a Actor, permission, owner, team string, sensitive bool) error {
	if a.UserID == "" {
		return Unauthorized
	}
	var active bool
	var secret sql.NullString
	suffix := ""
	if _, ok := q.(*sql.Tx); ok {
		suffix = " FOR UPDATE"
	}
	if e := q.QueryRowContext(ctx, "SELECT active,totp_secret FROM users WHERE id=?"+suffix, a.UserID).Scan(&active, &secret); e != nil {
		if e == sql.ErrNoRows {
			return Unauthorized
		}
		return e
	}
	if !active || !secret.Valid {
		return Forbidden
	}
	if a.Type == "user" {
		var mfa bool
		var expires, last time.Time
		var reauth sql.NullTime
		if e := q.QueryRowContext(ctx, "SELECT mfa_complete,expires_at,last_activity,reauthenticated_at FROM sessions WHERE token_hash=? AND user_id=?", a.SessionHash, a.UserID).Scan(&mfa, &expires, &last, &reauth); e != nil {
			return Unauthorized
		}
		if !mfa || time.Now().After(expires) || time.Since(last) > s.Config.Idle {
			return Unauthorized
		}
		if sensitive && (!reauth.Valid || time.Since(reauth.Time) > s.Config.Reauth) {
			return &Error{"reauth_required", "Confirme su contraseña y segundo factor.", 403}
		}
	} else if a.Type == "service" {
		var enabled bool
		var expires time.Time
		var raw string
		if e := q.QueryRowContext(ctx, "SELECT active,expires_at,scopes FROM service_identities WHERE id=? AND user_id=?", a.ServiceID, a.UserID).Scan(&enabled, &expires, &raw); e != nil {
			return Unauthorized
		}
		var scopes []string
		if json.Unmarshal([]byte(raw), &scopes) != nil {
			return Forbidden
		}
		if !enabled || time.Now().After(expires) || !s.servicePermissionAllowed(permission) || !contains(scopes, permission) || sensitive {
			return Forbidden
		}
	} else {
		return Forbidden
	}
	if permission == "core.identity.self" && a.Type == "user" {
		return nil
	}
	return s.authorizeScope(ctx, q, a.UserID, permission, owner, team)
}

// AuthorizeGrantHolder verifies a durable decision holder without treating an
// old session as current authentication. This is not an HTTP execution identity.
func (s *Service) AuthorizeGrantHolder(ctx context.Context, q Querier, user, permission, owner, team string) error {
	if ID(user) != nil {
		return Forbidden
	}
	var active bool
	var secret sql.NullString
	e := q.QueryRowContext(ctx, "SELECT active,totp_secret FROM users WHERE id=?", user).Scan(&active, &secret)
	if e == sql.ErrNoRows {
		return Forbidden
	}
	if e != nil {
		return e
	}
	if !active || !secret.Valid {
		return Forbidden
	}
	return s.authorizeScope(ctx, q, user, permission, owner, team)
}

func (s *Service) authorizeScope(ctx context.Context, q Querier, user, permission, owner, team string) error {
	grants, e := s.Grants(ctx, q, user)
	if e != nil {
		return e
	}
	for _, g := range grants {
		if g.Permission != permission {
			continue
		}
		switch g.Scope {
		case "all":
			return nil
		case "self":
			if owner != "" && owner == user {
				return nil
			}
		case "team":
			if team != "" {
				var n int
				if e = q.QueryRowContext(ctx, "SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?", team, user).Scan(&n); e != nil {
					return e
				}
				if n > 0 {
					return nil
				}
			}
		}
	}
	return Forbidden
}
func (s *Service) Grants(ctx context.Context, q Querier, user string) ([]Grant, error) {
	rows, e := q.QueryContext(ctx, `SELECT DISTINCT rp.permission,rp.scope FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? ORDER BY rp.permission,rp.scope`, user)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Grant{}
	for rows.Next() {
		var g Grant
		if e = rows.Scan(&g.Permission, &g.Scope); e != nil {
			return nil, e
		}
		out = append(out, g)
	}
	return out, rows.Err()
}
func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}
func (s *Service) Profile(ctx context.Context, user string) (User, error) {
	var u User
	var secret sql.NullString
	e := s.DB.QueryRowContext(ctx, "SELECT id,email,name,active,totp_secret,version FROM users WHERE id=?", user).Scan(&u.ID, &u.Email, &u.Name, &u.Active, &secret, &u.Version)
	u.MFA = secret.Valid
	if e != nil {
		return u, e
	}
	u.Roles = []string{}
	rows, e := s.DB.QueryContext(ctx, "SELECT role_id FROM user_roles WHERE user_id=? ORDER BY role_id", user)
	if e != nil {
		return u, e
	}
	defer rows.Close()
	for rows.Next() {
		var id string
		if e = rows.Scan(&id); e != nil {
			return u, e
		}
		u.Roles = append(u.Roles, id)
	}
	return u, rows.Err()
}
func (s *Service) Idempotent(ctx context.Context, tx *sql.Tx, a Actor, op, key string, input any, fn func() (any, error)) (any, error) {
	if len(key) < 8 || len(key) > 100 || strings.IndexFunc(key, func(r rune) bool { return r < 33 || r > 126 }) >= 0 {
		return nil, Invalid
	}
	body, e := json.Marshal(input)
	if e != nil {
		return nil, e
	}
	hash := security.Hash(string(body))
	_, e = tx.ExecContext(ctx, `INSERT INTO idempotency_records(actor_key,operation,request_key,request_hash,response,created_at) VALUES(?,?,?,?,JSON_OBJECT(),UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE request_key=VALUES(request_key)`, a.Key(), op, key, hash)
	if e != nil {
		return nil, e
	}
	var oldHash, response string
	if e = tx.QueryRowContext(ctx, `SELECT request_hash,response FROM idempotency_records WHERE actor_key=? AND operation=? AND request_key=? FOR UPDATE`, a.Key(), op, key).Scan(&oldHash, &response); e != nil {
		return nil, e
	}
	if oldHash != hash {
		return nil, Conflict
	}
	var prior map[string]json.RawMessage
	if e = json.Unmarshal([]byte(response), &prior); e != nil {
		return nil, e
	}
	if len(prior) > 0 {
		return json.RawMessage(response), nil
	}
	result, e := fn()
	if e != nil {
		return nil, e
	}
	encoded, e := json.Marshal(result)
	if e != nil {
		return nil, e
	}
	if _, e = tx.ExecContext(ctx, `UPDATE idempotency_records SET response=? WHERE actor_key=? AND operation=? AND request_key=?`, string(encoded), a.Key(), op, key); e != nil {
		return nil, e
	}
	return result, nil
}
func IsClient(e error) bool { var ce *Error; return errors.As(e, &ce) }
func Changed(r sql.Result, e error) error {
	if e != nil {
		return e
	}
	n, e := r.RowsAffected()
	if e != nil {
		return e
	}
	if n != 1 {
		return Conflict
	}
	return nil
}
func WrapID(n int64) string { return fmt.Sprint(n) }
