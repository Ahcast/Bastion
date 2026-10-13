package core

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"net/mail"
	"strings"
	"time"
)

func (s *Service) NewSession(ctx context.Context, user string, mfa bool) (token, csrf string, err error) {
	token, csrf = security.Token(), security.Token()
	_, err = s.DB.ExecContext(ctx, `INSERT INTO sessions(token_hash,csrf_hash,user_id,mfa_complete,created_at,last_activity,expires_at) VALUES(?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),?)`, security.Hash(token), security.Hash(csrf), nullable(user), mfa, time.Now().UTC().Add(s.Config.Absolute))
	return
}
func (s *Service) Session(ctx context.Context, token string) (Session, error) {
	var x Session
	var uid sql.NullString
	if token == "" {
		return x, Unauthorized
	}
	x.Hash = security.Hash(token)
	e := s.DB.QueryRowContext(ctx, "SELECT csrf_hash,user_id,mfa_complete,created_at,last_activity,expires_at,reauthenticated_at FROM sessions WHERE token_hash=?", x.Hash).Scan(&x.CSRFHash, &uid, &x.MFA, &x.Created, &x.Last, &x.Expires, &x.Reauthenticated)
	if e != nil {
		if e == sql.ErrNoRows {
			return x, Unauthorized
		}
		return x, e
	}
	x.UserID = uid.String
	if time.Now().After(x.Expires) || time.Since(x.Last) > s.Config.Idle {
		return x, Unauthorized
	}
	if x.UserID != "" {
		var active bool
		if e = s.DB.QueryRowContext(ctx, "SELECT active FROM users WHERE id=?", x.UserID).Scan(&active); e != nil {
			return x, Unauthorized
		}
		if !active {
			return x, Unauthorized
		}
	}
	return x, nil
}
func (s *Service) RotateCSRF(ctx context.Context, x Session) (string, error) {
	token := security.Token()
	return token, Changed(s.DB.ExecContext(ctx, "UPDATE sessions SET csrf_hash=? WHERE token_hash=?", security.Hash(token), x.Hash))
}
func (s *Service) Touch(ctx context.Context, x Session) error {
	return Changed(s.DB.ExecContext(ctx, "UPDATE sessions SET last_activity=UTC_TIMESTAMP(6) WHERE token_hash=?", x.Hash))
}
func (s *Service) Limit(ctx context.Context, bucket string, max int) error {
	return s.Transact(ctx, func(tx *sql.Tx) error {
		hash := security.Hash(bucket)
		// Reserve the bucket with an exclusive lock. INSERT IGNORE would let
		// concurrent callers hold shared duplicate-key locks, then deadlock when
		// both try to promote them at SELECT FOR UPDATE.
		if _, e := tx.ExecContext(ctx, "INSERT INTO auth_limits VALUES(?,0,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE bucket=VALUES(bucket)", hash); e != nil {
			return e
		}
		var count int
		var start time.Time
		if e := tx.QueryRowContext(ctx, "SELECT attempts,window_start FROM auth_limits WHERE bucket=? FOR UPDATE", hash).Scan(&count, &start); e != nil {
			return e
		}
		if time.Since(start) > 15*time.Minute {
			_, e := tx.ExecContext(ctx, "UPDATE auth_limits SET attempts=1,window_start=UTC_TIMESTAMP(6) WHERE bucket=?", hash)
			return e
		}
		if count >= max {
			return &Error{"rate_limited", "Demasiados intentos. Intente más tarde.", 429}
		}
		_, e := tx.ExecContext(ctx, "UPDATE auth_limits SET attempts=attempts+1 WHERE bucket=?", hash)
		return e
	})
}
func (s *Service) Login(ctx context.Context, x Session, a Actor, email, password string) (string, string, error) {
	email = strings.ToLower(strings.TrimSpace(email))
	if e := s.Limit(ctx, "login-email:"+email, 10); e != nil {
		return "", "", e
	}
	var id, hash string
	var active bool
	e := s.DB.QueryRowContext(ctx, "SELECT id,password_hash,active FROM users WHERE email=?", email).Scan(&id, &hash, &active)
	if e != nil && e != sql.ErrNoRows {
		return "", "", e
	}
	if e == sql.ErrNoRows {
		hash = s.DummyHash
	}
	valid := security.Verify(password, hash)
	if !valid || !active || id == "" {
		_ = s.Audit(ctx, s.DB, a, "auth.login", "user", "", "denied", map[string]string{})
		return "", "", Unauthorized
	}
	token, csrf := security.Token(), security.Token()
	e = s.Transact(ctx, func(tx *sql.Tx) error {
		var current string
		var enabled bool
		if e := tx.QueryRowContext(ctx, "SELECT password_hash,active FROM users WHERE id=? FOR UPDATE", id).Scan(&current, &enabled); e != nil {
			return e
		}
		if !enabled || current != hash {
			return Unauthorized
		}
		if e := Changed(tx.ExecContext(ctx, "DELETE FROM sessions WHERE token_hash=?", x.Hash)); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, `INSERT INTO sessions(token_hash,csrf_hash,user_id,mfa_complete,created_at,last_activity,expires_at) VALUES(?,?,?,FALSE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),?)`, security.Hash(token), security.Hash(csrf), id, time.Now().UTC().Add(s.Config.Absolute)); e != nil {
			return e
		}
		a.ID = id
		a.UserID = id
		return s.Audit(ctx, tx, a, "auth.login", "user", id, "partial", map[string]string{})
	})
	return token, csrf, e
}
func (s *Service) Enroll(ctx context.Context, x Session) (string, error) {
	if x.UserID == "" || x.MFA {
		return "", Forbidden
	}
	u, e := s.Profile(ctx, x.UserID)
	if e != nil {
		return "", e
	}
	secret, uri, e := security.NewTOTP(u.Email)
	if e != nil {
		return "", e
	}
	encrypted, e := security.Encrypt(s.Config.Key, secret)
	if e != nil {
		return "", e
	}
	e = s.Transact(ctx, func(tx *sql.Tx) error {
		var existing sql.NullString
		var active bool
		if e := tx.QueryRowContext(ctx, "SELECT totp_secret,active FROM users WHERE id=? FOR UPDATE", x.UserID).Scan(&existing, &active); e != nil {
			return e
		}
		if existing.Valid || !active {
			return Forbidden
		}
		if _, e := tx.ExecContext(ctx, "UPDATE users SET totp_pending=? WHERE id=?", encrypted, x.UserID); e != nil {
			return e
		}
		return nil
	})
	return uri, e
}
func (s *Service) Factor(ctx context.Context, tx *sql.Tx, user, code string, enroll bool) error {
	var encrypted, pending sql.NullString
	var last int64
	var active bool
	if e := tx.QueryRowContext(ctx, "SELECT totp_secret,totp_pending,totp_last_step,active FROM users WHERE id=? FOR UPDATE", user).Scan(&encrypted, &pending, &last, &active); e != nil {
		return e
	}
	if !active {
		return Unauthorized
	}
	if enroll {
		if encrypted.Valid || !pending.Valid {
			return Forbidden
		}
		encrypted = pending
	}
	if !encrypted.Valid {
		return Forbidden
	}
	secret, e := security.Decrypt(s.Config.Key, encrypted.String)
	if e != nil {
		return e
	}
	step, ok := security.TOTPStep(secret, code, time.Now())
	if ok && step > last {
		_, e = tx.ExecContext(ctx, "UPDATE users SET totp_last_step=? WHERE id=?", step, user)
		return e
	}
	if !enroll && len(code) >= 20 {
		r, e := tx.ExecContext(ctx, "DELETE FROM recovery_codes WHERE user_id=? AND code_hash=?", user, security.Hash(code))
		if e != nil {
			return e
		}
		n, _ := r.RowsAffected()
		if n == 1 {
			return nil
		}
	}
	return Unauthorized
}
func (s *Service) CompleteMFA(ctx context.Context, x Session, a Actor, code string, enroll bool) (token, csrf string, codes []string, err error) {
	if x.UserID == "" || x.MFA {
		return "", "", nil, Forbidden
	}
	if e := s.Limit(ctx, "factor:"+x.UserID, 10); e != nil {
		return "", "", nil, e
	}
	token, csrf = security.Token(), security.Token()
	err = s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Factor(ctx, tx, x.UserID, code, enroll); e != nil {
			return e
		}
		if enroll {
			if _, e := tx.ExecContext(ctx, "UPDATE users SET totp_secret=totp_pending,totp_pending=NULL,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=?", x.UserID); e != nil {
				return e
			}
			for i := 0; i < 8; i++ {
				c := security.Token()
				codes = append(codes, c)
				if _, e := tx.ExecContext(ctx, "INSERT INTO recovery_codes VALUES(?,?)", x.UserID, security.Hash(c)); e != nil {
					return e
				}
			}
		}
		if e := Changed(tx.ExecContext(ctx, "DELETE FROM sessions WHERE token_hash=? AND mfa_complete=FALSE", x.Hash)); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, `INSERT INTO sessions(token_hash,csrf_hash,user_id,mfa_complete,created_at,last_activity,expires_at,reauthenticated_at) VALUES(?,?,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),?,UTC_TIMESTAMP(6))`, security.Hash(token), security.Hash(csrf), x.UserID, x.Expires); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "auth.mfa", "user", x.UserID, "success", map[string]bool{"enrollment": enroll})
	})
	if err != nil {
		codes = nil
		_ = s.Audit(ctx, s.DB, a, "auth.mfa", "user", x.UserID, "denied", map[string]string{})
	}
	return
}
func (s *Service) Reauthenticate(ctx context.Context, x Session, a Actor, password, code string) error {
	if !x.MFA {
		return Forbidden
	}
	if e := s.Limit(ctx, "reauth:"+x.UserID, 10); e != nil {
		return e
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		var hash string
		if e := tx.QueryRowContext(ctx, "SELECT password_hash FROM users WHERE id=? FOR UPDATE", x.UserID).Scan(&hash); e != nil {
			return e
		}
		if !security.Verify(password, hash) {
			return Unauthorized
		}
		if e := s.Factor(ctx, tx, x.UserID, code, false); e != nil {
			return e
		}
		if e := Changed(tx.ExecContext(ctx, "UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?", x.Hash)); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "auth.reauth", "user", x.UserID, "success", map[string]string{})
	})
}
func (s *Service) Logout(ctx context.Context, x Session, a Actor) error {
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if _, e := tx.ExecContext(ctx, "DELETE FROM sessions WHERE token_hash=?", x.Hash); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "auth.logout", "session", "", "success", map[string]string{})
	})
}
func ValidEmail(email string) bool {
	a, e := mail.ParseAddress(email)
	return e == nil && a.Address == email && len(email) <= 254 && strings.IndexFunc(email, func(r rune) bool { return r > 127 }) < 0
}
func (s *Service) Bootstrap(ctx context.Context, email, name, password string) error {
	if !ValidEmail(email) || len(name) < 1 || len(name) > 120 || !security.ValidPassword(password) {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if _, e := tx.ExecContext(ctx, "INSERT INTO bootstrap_guard VALUES(1)"); e != nil {
			return Conflict
		}
		var n int
		if e := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM users").Scan(&n); e != nil {
			return e
		}
		if n != 0 {
			return Conflict
		}
		for _, p := range Permissions {
			if _, e := tx.ExecContext(ctx, "INSERT IGNORE INTO permissions VALUES(?)", p); e != nil {
				return e
			}
		}
		r, e := tx.ExecContext(ctx, "INSERT INTO roles(id,name) VALUES(1,'Administración del sistema')")
		if e != nil {
			return e
		}
		rid, _ := r.LastInsertId()
		r, e = tx.ExecContext(ctx, `INSERT INTO users(email,name,password_hash,created_at,updated_at) VALUES(?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, strings.ToLower(email), name, security.Password(password))
		if e != nil {
			return e
		}
		uid, _ := r.LastInsertId()
		if _, e = tx.ExecContext(ctx, "INSERT INTO user_roles VALUES(?,?)", uid, rid); e != nil {
			return e
		}
		for _, p := range Permissions {
			if _, e = tx.ExecContext(ctx, "INSERT INTO role_permissions VALUES(?,?,'all')", rid, p); e != nil {
				return e
			}
		}
		return s.Audit(ctx, tx, Actor{Type: "user", ID: WrapID(uid), UserID: WrapID(uid), Source: "cli", RequestID: security.ID(), CorrelationID: security.ID()}, "identity.bootstrap", "user", WrapID(uid), "success", map[string]string{})
	})
}
func (s *Service) CreateIdentityToken(ctx context.Context, a Actor, kind, email string) (any, error) {
	email = strings.ToLower(strings.TrimSpace(email))
	if !ValidEmail(email) || (kind != "invite" && kind != "reset") {
		return nil, Invalid
	}
	token := security.Token()
	var result any
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.users.manage", "", "", true); e != nil {
			return e
		}
		var uid string
		e := tx.QueryRowContext(ctx, "SELECT id FROM users WHERE email=?", email).Scan(&uid)
		if kind == "invite" {
			if e == nil {
				return Conflict
			}
			if e != sql.ErrNoRows {
				return e
			}
		} else {
			if e == sql.ErrNoRows {
				return Missing
			}
			if e != nil {
				return e
			}
			if e = s.protectSystemAdministrator(ctx, tx, uid); e != nil {
				return e
			}
		}
		expires := time.Now().UTC().Add(24 * time.Hour)
		if _, e = tx.ExecContext(ctx, "UPDATE identity_tokens SET consumed_at=UTC_TIMESTAMP(6) WHERE email=? AND kind=? AND consumed_at IS NULL", email, kind); e != nil {
			return e
		}
		if _, e = tx.ExecContext(ctx, "INSERT INTO identity_tokens(token_hash,kind,email,user_id,created_by,expires_at) VALUES(?,?,?,?,?,?)", security.Hash(token), kind, email, nullable(uid), a.UserID, expires); e != nil {
			return e
		}
		result = map[string]any{"token": token, "kind": kind, "expires_at": expires}
		return s.Audit(ctx, tx, a, "identity."+kind, "user", uid, "success", map[string]string{})
	})
	return result, e
}
func (s *Service) AcceptIdentityToken(ctx context.Context, token, name, password string) error {
	if !security.ValidPassword(password) || len(name) > 120 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		var kind, email string
		var uid sql.NullString
		var expires time.Time
		var consumed sql.NullTime
		var creator string
		if e := tx.QueryRowContext(ctx, "SELECT created_by FROM identity_tokens WHERE token_hash=?", security.Hash(token)).Scan(&creator); e != nil {
			return Unauthorized
		}
		var creatorActive bool
		if e := tx.QueryRowContext(ctx, "SELECT active FROM users WHERE id=? FOR UPDATE", creator).Scan(&creatorActive); e != nil || !creatorActive {
			return Unauthorized
		}
		var allowed int
		if e := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='core.users.manage' AND rp.scope='all'", creator).Scan(&allowed); e != nil {
			return e
		}
		if allowed == 0 {
			return Forbidden
		}
		if e := tx.QueryRowContext(ctx, "SELECT kind,email,user_id,expires_at,consumed_at FROM identity_tokens WHERE token_hash=? FOR UPDATE", security.Hash(token)).Scan(&kind, &email, &uid, &expires, &consumed); e != nil {
			return Unauthorized
		}
		if consumed.Valid || time.Now().After(expires) {
			return Unauthorized
		}
		if kind == "reset" && uid.Valid {
			if e := s.protectSystemAdministrator(ctx, tx, uid.String); e != nil {
				return e
			}
		}
		var user string
		if kind == "invite" {
			if name == "" {
				return Invalid
			}
			r, e := tx.ExecContext(ctx, "INSERT INTO users(email,name,password_hash,created_at,updated_at) VALUES(?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))", email, name, security.Password(password))
			if e != nil {
				return e
			}
			n, _ := r.LastInsertId()
			user = WrapID(n)
		} else {
			user = uid.String
			if e := Changed(tx.ExecContext(ctx, "UPDATE users SET password_hash=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND active=TRUE", security.Password(password), user)); e != nil {
				return e
			}
			if _, e := tx.ExecContext(ctx, "DELETE FROM sessions WHERE user_id=?", user); e != nil {
				return e
			}
		}
		if _, e := tx.ExecContext(ctx, "UPDATE identity_tokens SET consumed_at=UTC_TIMESTAMP(6) WHERE token_hash=?", security.Hash(token)); e != nil {
			return e
		}
		return s.Audit(ctx, tx, Actor{Type: "user", ID: user, UserID: user, Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}, "identity.accept_"+kind, "user", user, "success", map[string]string{})
	})
}
func (s *Service) AuthenticateService(ctx context.Context, token string) (Actor, error) {
	var a Actor
	var scopes string
	var active bool
	var expires time.Time
	if e := s.DB.QueryRowContext(ctx, "SELECT id,user_id,scopes,active,expires_at FROM service_identities WHERE token_hash=?", security.Hash(token)).Scan(&a.ID, &a.UserID, &scopes, &active, &expires); e != nil {
		return a, Unauthorized
	}
	if !active || time.Now().After(expires) {
		return a, Unauthorized
	}
	a.Type = "service"
	a.ServiceID = a.ID
	if e := json.Unmarshal([]byte(scopes), &a.Scopes); e != nil {
		return a, e
	}
	return a, nil
}
