package core

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"strings"
	"time"
)

type UserChange struct {
	Name    string   `json:"name"`
	Active  bool     `json:"active"`
	Roles   []string `json:"roles"`
	Version uint64   `json:"version,string"`
}

func (s *Service) Users(ctx context.Context, a Actor, limit, offset int) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.users.read", "", "", false); e != nil {
		return nil, e
	}
	rows, e := s.DB.QueryContext(ctx, "SELECT id,email,name,active,totp_secret IS NOT NULL,version FROM users ORDER BY id LIMIT ? OFFSET ?", limit, offset)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []User{}
	for rows.Next() {
		var u User
		if e = rows.Scan(&u.ID, &u.Email, &u.Name, &u.Active, &u.MFA, &u.Version); e != nil {
			return nil, e
		}
		out = append(out, u)
	}
	if e = rows.Err(); e != nil {
		return nil, e
	}
	rows.Close()
	for i := range out {
		roles, e := s.DB.QueryContext(ctx, "SELECT role_id FROM user_roles WHERE user_id=? ORDER BY role_id", out[i].ID)
		if e != nil {
			return nil, e
		}
		out[i].Roles = []string{}
		for roles.Next() {
			var id string
			if e = roles.Scan(&id); e != nil {
				roles.Close()
				return nil, e
			}
			out[i].Roles = append(out[i].Roles, id)
		}
		e = roles.Err()
		roles.Close()
		if e != nil {
			return nil, e
		}
	}
	return map[string]any{"data": out}, nil
}
func (s *Service) UpdateUser(ctx context.Context, a Actor, id string, c UserChange) error {
	if ID(id) != nil || len(c.Name) < 1 || len(c.Name) > 120 || len(c.Roles) > 30 || id == a.UserID {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.users.manage", "", "", true); e != nil {
			return e
		}
		if e := s.Authorize(ctx, tx, a, "core.roles.manage", "", "", true); e != nil {
			return e
		}
		var protected int
		if e := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM user_roles WHERE user_id=? AND role_id=1", id).Scan(&protected); e != nil {
			return e
		}
		if protected > 0 {
			return Forbidden
		}
		if e := Changed(tx.ExecContext(ctx, "UPDATE users SET name=?,active=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?", c.Name, c.Active, id, c.Version)); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM user_roles WHERE user_id=?", id); e != nil {
			return e
		}
		for _, rid := range c.Roles {
			if ID(rid) != nil || rid == "1" {
				return Invalid
			}
			if _, e := tx.ExecContext(ctx, "INSERT INTO user_roles VALUES(?,?)", id, rid); e != nil {
				return e
			}
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM sessions WHERE user_id=?", id); e != nil {
			return e
		}
		if !c.Active {
			if _, e := tx.ExecContext(ctx, "UPDATE service_identities SET active=FALSE WHERE user_id=?", id); e != nil {
				return e
			}
		}
		return s.Audit(ctx, tx, a, "users.update", "user", id, "success", map[string]any{"active": c.Active, "roles": c.Roles})
	})
}
func (s *Service) ResetMFA(ctx context.Context, a Actor, id, verification string) error {
	if ID(id) != nil || id == a.UserID || len(strings.TrimSpace(verification)) < 10 || len(verification) > 200 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.users.manage", "", "", true); e != nil {
			return e
		}
		if e := Changed(tx.ExecContext(ctx, "UPDATE users SET totp_secret=NULL,totp_pending=NULL,totp_last_step=-1,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=?", id)); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM recovery_codes WHERE user_id=?", id); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM sessions WHERE user_id=?", id); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "UPDATE service_identities SET active=FALSE WHERE user_id=?", id); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "users.reset_mfa", "user", id, "success", map[string]string{"verification_reference": verification})
	})
}
func (s *Service) RevokeSessions(ctx context.Context, a Actor, id string) error {
	if ID(id) != nil {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.users.manage", "", "", true); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM sessions WHERE user_id=?", id); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "sessions.revoke", "user", id, "success", map[string]string{})
	})
}

type Role struct {
	ID      string  `json:"id"`
	Name    string  `json:"name"`
	Version uint64  `json:"version,string"`
	Grants  []Grant `json:"grants"`
}

func (s *Service) Roles(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.roles.read", "", "", false); e != nil {
		return nil, e
	}
	rows, e := s.DB.QueryContext(ctx, "SELECT id,name,version FROM roles ORDER BY id LIMIT 100")
	if e != nil {
		return nil, e
	}
	out := []Role{}
	for rows.Next() {
		var r Role
		if e = rows.Scan(&r.ID, &r.Name, &r.Version); e != nil {
			rows.Close()
			return nil, e
		}
		out = append(out, r)
	}
	if e = rows.Err(); e != nil {
		rows.Close()
		return nil, e
	}
	rows.Close()
	for i := range out {
		rows, e = s.DB.QueryContext(ctx, "SELECT permission,scope FROM role_permissions WHERE role_id=? ORDER BY permission", out[i].ID)
		if e != nil {
			return nil, e
		}
		out[i].Grants = []Grant{}
		for rows.Next() {
			var g Grant
			if e = rows.Scan(&g.Permission, &g.Scope); e != nil {
				rows.Close()
				return nil, e
			}
			out[i].Grants = append(out[i].Grants, g)
		}
		e = rows.Err()
		rows.Close()
		if e != nil {
			return nil, e
		}
	}
	return map[string]any{"data": out, "permissions": s.PermissionCatalog()}, nil
}
func (s *Service) SaveRole(ctx context.Context, a Actor, id string, c Role) (any, error) {
	if len(c.Name) < 1 || len(c.Name) > 80 || len(c.Grants) > 100 || id == "1" {
		return nil, Invalid
	}
	for _, g := range c.Grants {
		if !contains(s.PermissionCatalog(), g.Permission) || (g.Scope != "all" && g.Scope != "self" && g.Scope != "team") {
			return nil, Invalid
		}
		if g.Scope != "all" && strings.HasPrefix(g.Permission, "core.") && !strings.HasPrefix(g.Permission, "core.documents.") && g.Permission != "core.notifications.read" {
			return nil, Invalid
		}
	}
	var result any
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.roles.manage", "", "", true); e != nil {
			return e
		}
		if id == "" {
			r, e := tx.ExecContext(ctx, "INSERT INTO roles(name) VALUES(?)", c.Name)
			if e != nil {
				return e
			}
			n, _ := r.LastInsertId()
			id = WrapID(n)
		} else {
			if ID(id) != nil {
				return Invalid
			}
			if e := Changed(tx.ExecContext(ctx, "UPDATE roles SET name=?,version=version+1 WHERE id=? AND version=?", c.Name, id, c.Version)); e != nil {
				return e
			}
			if _, e := tx.ExecContext(ctx, "DELETE FROM role_permissions WHERE role_id=?", id); e != nil {
				return e
			}
		}
		for _, g := range c.Grants {
			if _, e := tx.ExecContext(ctx, "INSERT INTO role_permissions VALUES(?,?,?)", id, g.Permission, g.Scope); e != nil {
				return e
			}
		}
		if _, e := tx.ExecContext(ctx, "DELETE s FROM sessions s JOIN user_roles ur ON ur.user_id=s.user_id WHERE ur.role_id=? AND s.user_id<>?", id, a.UserID); e != nil {
			return e
		}
		result = map[string]string{"id": id}
		return s.Audit(ctx, tx, a, "roles.save", "role", id, "success", map[string]any{"grants": c.Grants})
	})
	return result, e
}

type Team struct {
	ID      string   `json:"id"`
	Name    string   `json:"name"`
	Version uint64   `json:"version,string"`
	Members []string `json:"members"`
}

func (s *Service) Teams(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.teams.read", "", "", false); e != nil {
		return nil, e
	}
	rows, e := s.DB.QueryContext(ctx, "SELECT id,name,version FROM teams ORDER BY id LIMIT 100")
	if e != nil {
		return nil, e
	}
	out := []Team{}
	for rows.Next() {
		var t Team
		if e = rows.Scan(&t.ID, &t.Name, &t.Version); e != nil {
			rows.Close()
			return nil, e
		}
		out = append(out, t)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return nil, e
	}
	for i := range out {
		rows, e = s.DB.QueryContext(ctx, "SELECT user_id FROM team_members WHERE team_id=? ORDER BY user_id", out[i].ID)
		if e != nil {
			return nil, e
		}
		out[i].Members = []string{}
		for rows.Next() {
			var id string
			if e = rows.Scan(&id); e != nil {
				rows.Close()
				return nil, e
			}
			out[i].Members = append(out[i].Members, id)
		}
		e = rows.Err()
		rows.Close()
		if e != nil {
			return nil, e
		}
	}
	return map[string]any{"data": out}, nil
}
func (s *Service) SaveTeam(ctx context.Context, a Actor, id string, c Team) (any, error) {
	if len(c.Name) < 1 || len(c.Name) > 80 || len(c.Members) > 100 {
		return nil, Invalid
	}
	var result any
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.teams.manage", "", "", true); e != nil {
			return e
		}
		if id == "" {
			r, e := tx.ExecContext(ctx, "INSERT INTO teams(name) VALUES(?)", c.Name)
			if e != nil {
				return e
			}
			n, _ := r.LastInsertId()
			id = WrapID(n)
		} else {
			if ID(id) != nil {
				return Invalid
			}
			if e := Changed(tx.ExecContext(ctx, "UPDATE teams SET name=?,version=version+1 WHERE id=? AND version=?", c.Name, id, c.Version)); e != nil {
				return e
			}
			if _, e := tx.ExecContext(ctx, "DELETE FROM team_members WHERE team_id=?", id); e != nil {
				return e
			}
		}
		for _, u := range c.Members {
			if ID(u) != nil {
				return Invalid
			}
			if _, e := tx.ExecContext(ctx, "INSERT INTO team_members VALUES(?,?)", id, u); e != nil {
				return e
			}
		}
		result = map[string]string{"id": id}
		return s.Audit(ctx, tx, a, "teams.save", "team", id, "success", map[string]any{"members": c.Members})
	})
	return result, e
}

type Setting struct {
	Name      string          `json:"name"`
	Value     json.RawMessage `json:"value"`
	HasSecret bool            `json:"has_secret"`
	Version   uint64          `json:"version,string"`
}
type SMTPSettings struct {
	Host     string `json:"host"`
	Port     int    `json:"port"`
	Username string `json:"username"`
	From     string `json:"from"`
	TLS      string `json:"tls"`
}
type CompanySettings struct {
	Name    string `json:"name"`
	TaxID   string `json:"tax_id"`
	Address string `json:"address"`
}

func (s *Service) Settings(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.settings.read", "", "", false); e != nil {
		return nil, e
	}
	rows, e := s.DB.QueryContext(ctx, "SELECT name,value,secret IS NOT NULL,version FROM settings ORDER BY name LIMIT 100")
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Setting{}
	for rows.Next() {
		var x Setting
		if e = rows.Scan(&x.Name, &x.Value, &x.HasSecret, &x.Version); e != nil {
			return nil, e
		}
		out = append(out, x)
	}
	return map[string]any{"data": out}, rows.Err()
}
func (s *Service) SaveSetting(ctx context.Context, a Actor, name string, c Setting, secret string) error {
	if name == "company" {
		var v CompanySettings
		if e := strictValue(c.Value, &v); e != nil || v.Name == "" || len(v.Name) > 120 || len(v.TaxID) > 40 || len(v.Address) > 500 || secret != "" {
			return Invalid
		}
	} else if name == "integrations" {
		var v Integrations
		if strictValue(c.Value, &v) != nil || secret != "" {
			return Invalid
		}
	} else if name == "smtp" {
		var v SMTPSettings
		if e := strictValue(c.Value, &v); e != nil || !contains(s.Config.SMTPHosts, v.Host) || !ValidEmail(v.From) || len(v.Username) > 254 || !(v.Port == 465 && v.TLS == "implicit" || v.Port == 587 && v.TLS == "starttls") {
			return Invalid
		}
	} else {
		return Invalid
	}
	if len(secret) > 512 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.settings.manage", "", "", true); e != nil {
			return e
		}
		var encrypted any
		if secret != "" {
			v, e := security.Encrypt(s.Config.Key, secret)
			if e != nil {
				return e
			}
			encrypted = v
		}
		if c.Version == 0 {
			if _, e := tx.ExecContext(ctx, "INSERT INTO settings(name,value,secret,updated_at) VALUES(?,?,?,UTC_TIMESTAMP(6))", name, string(c.Value), encrypted); e != nil {
				return e
			}
		} else {
			if e := Changed(tx.ExecContext(ctx, "UPDATE settings SET value=?,secret=COALESCE(?,secret),version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE name=? AND version=?", string(c.Value), encrypted, name, c.Version)); e != nil {
				return e
			}
		}
		return s.Audit(ctx, tx, a, "settings.save", "setting", name, "success", map[string]bool{"secret_changed": secret != ""})
	})
}
func (s *Service) CreateService(ctx context.Context, a Actor, name string, scopes []string, expires time.Time) (any, error) {
	if len(name) < 1 || len(name) > 80 || len(scopes) == 0 || len(scopes) > 30 || !expires.After(time.Now()) || expires.After(time.Now().Add(90*24*time.Hour)) {
		return nil, Invalid
	}
	for _, p := range scopes {
		if !contains(s.PermissionCatalog(), p) {
			return nil, Invalid
		}
		if p != "core.modules.read" && p != "core.documents.read" && p != "core.jobs.read" && !contains(s.ServiceReadPermissions, p) && !contains(s.ServiceWritePermissions, p) {
			return nil, Forbidden
		}
	}
	token := security.Token()
	var result any
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.services.manage", "", "", true); e != nil {
			return e
		}
		var team string
		if e := tx.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team); e != nil && e != sql.ErrNoRows {
			return e
		}
		for _, p := range scopes {
			if e := s.Authorize(ctx, tx, a, p, a.UserID, team, false); e != nil {
				return e
			}
		}
		encoded, _ := json.Marshal(scopes)
		r, e := tx.ExecContext(ctx, "INSERT INTO service_identities(name,token_hash,user_id,scopes,expires_at) VALUES(?,?,?,?,?)", name, security.Hash(token), a.UserID, string(encoded), expires.UTC())
		if e != nil {
			return e
		}
		n, _ := r.LastInsertId()
		result = map[string]any{"id": WrapID(n), "token": token, "expires_at": expires}
		return s.Audit(ctx, tx, a, "services.create", "service", WrapID(n), "success", map[string]any{"scopes": scopes})
	})
	return result, e
}
func (s *Service) Services(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.services.manage", "", "", false); e != nil {
		return nil, e
	}
	return s.Rows(ctx, `SELECT CAST(id AS CHAR) id,name,CAST(user_id AS CHAR) user_id,active,scopes,expires_at FROM service_identities ORDER BY id LIMIT 100`)
}
func (s *Service) RevokeService(ctx context.Context, a Actor, id string) error {
	if ID(id) != nil {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.services.manage", "", "", true); e != nil {
			return e
		}
		if e := Changed(tx.ExecContext(ctx, "UPDATE service_identities SET active=FALSE WHERE id=?", id)); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "services.revoke", "service", id, "success", map[string]string{})
	})
}

// Rows only accepts SQL authored by the application, never client SQL.
func (s *Service) Rows(ctx context.Context, query string, args ...any) (any, error) {
	rows, e := s.DB.QueryContext(ctx, query, args...)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	columns, e := rows.Columns()
	if e != nil {
		return nil, e
	}
	out := []map[string]any{}
	for rows.Next() {
		v := make([]any, len(columns))
		p := make([]any, len(v))
		for i := range v {
			p[i] = &v[i]
		}
		if e = rows.Scan(p...); e != nil {
			return nil, e
		}
		m := map[string]any{}
		for i, k := range columns {
			switch x := v[i].(type) {
			case []byte:
				if k == "scopes" || k == "details" {
					var parsed any
					if err := json.Unmarshal(x, &parsed); err != nil {
						return nil, err
					}
					m[k] = parsed
				} else if k == "active" {
					m[k] = string(x) == "1"
				} else {
					m[k] = string(x)
				}
			case int64:
				if k == "active" {
					m[k] = x != 0
				} else {
					m[k] = x
				}
			default:
				m[k] = x
			}
		}
		out = append(out, m)
	}
	return map[string]any{"data": out}, rows.Err()
}
func (s *Service) AuditList(ctx context.Context, a Actor, limit, offset int) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.audit.read", "", "", false); e != nil {
		return nil, e
	}
	return s.Rows(ctx, `SELECT CAST(id AS CHAR) id,actor_type,actor_id,CAST(requested_by AS CHAR) requested_by,action,entity_type,entity_id,source,result,request_id,correlation_id,details,created_at FROM audit_events ORDER BY id DESC LIMIT ? OFFSET ?`, limit, offset)
}
func (s *Service) Notifications(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.notifications.read", a.UserID, "", false); e != nil {
		return nil, e
	}
	return s.Rows(ctx, "SELECT CAST(id AS CHAR) id,message,read_at,created_at FROM notifications WHERE user_id=? ORDER BY id DESC LIMIT 100", a.UserID)
}
func (s *Service) ReadNotification(ctx context.Context, a Actor, id string) error {
	if ID(id) != nil {
		return Invalid
	}
	if e := s.Authorize(ctx, s.DB, a, "core.notifications.read", a.UserID, "", false); e != nil {
		return e
	}
	_, e := s.DB.ExecContext(ctx, "UPDATE notifications SET read_at=UTC_TIMESTAMP(6) WHERE id=? AND user_id=?", id, a.UserID)
	return e
}

func strictValue(raw json.RawMessage, v any) error {
	d := json.NewDecoder(bytes.NewReader(raw))
	d.DisallowUnknownFields()
	if e := d.Decode(v); e != nil {
		return Invalid
	}
	return nil
}
