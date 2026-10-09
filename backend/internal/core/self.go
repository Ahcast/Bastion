package core

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

type Preferences struct {
	Theme    string `json:"theme"`
	PageSize int    `json:"page_size"`
}

func (s *Service) Preferences(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.identity.self", a.UserID, "", false); e != nil {
		return nil, e
	}
	p := Preferences{Theme: "system", PageSize: 25}
	var raw string
	e := s.DB.QueryRowContext(ctx, "SELECT value FROM user_preferences WHERE user_id=?", a.UserID).Scan(&raw)
	if e == sql.ErrNoRows {
		return p, nil
	}
	if e != nil {
		return nil, e
	}
	if e = json.Unmarshal([]byte(raw), &p); e != nil {
		return nil, e
	}
	return p, nil
}
func (s *Service) SavePreferences(ctx context.Context, a Actor, p Preferences) error {
	if (p.Theme != "system" && p.Theme != "light" && p.Theme != "dark") || (p.PageSize != 25 && p.PageSize != 50 && p.PageSize != 100) {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.identity.self", a.UserID, "", false); e != nil {
			return e
		}
		b, _ := json.Marshal(p)
		_, e := tx.ExecContext(ctx, "INSERT INTO user_preferences VALUES(?,?) ON DUPLICATE KEY UPDATE value=VALUES(value)", a.UserID, string(b))
		return e
	})
}
func (s *Service) RecoveryCodes(ctx context.Context, a Actor) ([]string, error) {
	codes := []string{}
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.identity.self", a.UserID, "", true); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM recovery_codes WHERE user_id=?", a.UserID); e != nil {
			return e
		}
		for i := 0; i < 8; i++ {
			c := security.Token()
			codes = append(codes, c)
			if _, e := tx.ExecContext(ctx, "INSERT INTO recovery_codes VALUES(?,?)", a.UserID, security.Hash(c)); e != nil {
				return e
			}
		}
		return s.Audit(ctx, tx, a, "identity.recovery_codes", "user", a.UserID, "success", map[string]string{})
	})
	if e != nil {
		return nil, e
	}
	return codes, nil
}
func (s *Service) ChangePassword(ctx context.Context, a Actor, password string) error {
	if !security.ValidPassword(password) {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.identity.self", a.UserID, "", true); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "UPDATE users SET password_hash=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=?", security.Password(password), a.UserID); e != nil {
			return e
		}
		if _, e := tx.ExecContext(ctx, "DELETE FROM sessions WHERE user_id=?", a.UserID); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "identity.password_change", "user", a.UserID, "success", map[string]string{})
	})
}
