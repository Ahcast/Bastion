package core

import (
	"context"
	"database/sql"
	"strings"
	"time"
)

type TrustedDevice struct {
	ID       string    `json:"id"`
	Name     string    `json:"name"`
	Created  time.Time `json:"created_at"`
	LastSeen time.Time `json:"last_seen_at"`
	Expires  time.Time `json:"expires_at"`
	Current  bool      `json:"current"`
}

func (s *Service) TrustedDevices(ctx context.Context, a Actor, limit, offset int) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.identity.self", a.UserID, "", false); e != nil {
		return nil, e
	}
	var total int
	if e := s.DB.QueryRowContext(ctx, "SELECT COUNT(*) FROM mfa_browser_verifications WHERE user_id=? AND expires_at>UTC_TIMESTAMP(6)", a.UserID).Scan(&total); e != nil {
		return nil, e
	}
	rows, e := s.DB.QueryContext(ctx, `SELECT device_id,device_name,created_at,last_seen_at,expires_at,IFNULL(token_hash=(SELECT browser_hash FROM sessions WHERE token_hash=?),FALSE) FROM mfa_browser_verifications WHERE user_id=? AND expires_at>UTC_TIMESTAMP(6) ORDER BY last_seen_at DESC,device_id LIMIT ? OFFSET ?`, a.SessionHash, a.UserID, limit, offset)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	data := []TrustedDevice{}
	for rows.Next() {
		var d TrustedDevice
		if e = rows.Scan(&d.ID, &d.Name, &d.Created, &d.LastSeen, &d.Expires, &d.Current); e != nil {
			return nil, e
		}
		data = append(data, d)
	}
	return map[string]any{"data": data, "total": total}, rows.Err()
}
func (s *Service) RevokeDevice(ctx context.Context, a Actor, id string) (bool, error) {
	if len(id) != 32 || strings.IndexFunc(id, func(r rune) bool { return !strings.ContainsRune("0123456789abcdef", r) }) >= 0 {
		return false, Invalid
	}
	current := false
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.identity.self", a.UserID, "", true); e != nil {
			return e
		}
		var hash string
		e := tx.QueryRowContext(ctx, "SELECT token_hash FROM mfa_browser_verifications WHERE device_id=? AND user_id=? FOR UPDATE", id, a.UserID).Scan(&hash)
		if e == sql.ErrNoRows {
			return Missing
		}
		if e != nil {
			return e
		}
		if e = tx.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash=? AND browser_hash=?)", a.SessionHash, hash).Scan(&current); e != nil {
			return e
		}
		if _, e = tx.ExecContext(ctx, "DELETE FROM mfa_browser_verifications WHERE token_hash=?", hash); e != nil {
			return e
		}
		if _, e = tx.ExecContext(ctx, "DELETE FROM sessions WHERE browser_hash=?", hash); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "auth.device_revoke", "device", id, "success", map[string]bool{"current": current})
	})
	return current, e
}

// A readable hint, never an authentication decision or a hardware identifier.
func BrowserName(ua string) string {
	browser, os := "Navegador", "sistema desconocido"
	switch {
	case strings.Contains(ua, "Edg/"):
		browser = "Edge"
	case strings.Contains(ua, "Firefox/") || strings.Contains(ua, "FxiOS/"):
		browser = "Firefox"
	case strings.Contains(ua, "Chrome/") || strings.Contains(ua, "CriOS/"):
		browser = "Chrome"
	case strings.Contains(ua, "Safari/"):
		browser = "Safari"
	}
	switch {
	case strings.Contains(ua, "Android"):
		os = "Android"
	case strings.Contains(ua, "iPhone") || strings.Contains(ua, "iPad"):
		os = "iOS"
	case strings.Contains(ua, "Windows"):
		os = "Windows"
	case strings.Contains(ua, "Macintosh"):
		os = "macOS"
	case strings.Contains(ua, "Linux"):
		os = "Linux"
	}
	return browser + " en " + os
}
