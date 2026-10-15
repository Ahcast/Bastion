package core

import (
	"context"
	"database/sql"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

const RememberedMFALifetime = 30 * 24 * time.Hour

type BrowserProof struct {
	IP, Token, UserAgent string
}

// SessionAtIP refuses a moved session before any application handler runs.
// Missing bindings on pre-migration sessions also require a fresh login/OTP.
func (s *Service) SessionAtIP(ctx context.Context, token, ip, browserToken string) (Session, error) {
	x, err := s.Session(ctx, token)
	if err != nil || !x.MFA {
		return x, err
	}
	if x.IPHash != "" && security.Equal(x.IPHash, security.Hash(ip)) {
		if x.BrowserHash != "" {
			if !security.Equal(x.BrowserHash, security.Hash(browserToken)) {
				return x, Unauthorized
			}
			var valid bool
			if err := s.DB.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM mfa_browser_verifications WHERE token_hash=? AND user_id=? AND ip_hash=? AND expires_at>UTC_TIMESTAMP(6))", x.BrowserHash, x.UserID, x.IPHash).Scan(&valid); err != nil {
				return x, err
			}
			if !valid {
				return x, Unauthorized
			}
		}
		return x, nil
	}
	err = s.Transact(ctx, func(tx *sql.Tx) error {
		var id string
		if err := tx.QueryRowContext(ctx, "SELECT id FROM users WHERE id=? FOR UPDATE", x.UserID).Scan(&id); err != nil {
			return err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM mfa_browser_verifications WHERE token_hash=?", x.BrowserHash); err != nil {
			return err
		}
		_, err := tx.ExecContext(ctx, "DELETE FROM sessions WHERE token_hash=? OR (browser_hash=? AND browser_hash IS NOT NULL)", x.Hash, nullable(x.BrowserHash))
		return err
	})
	if err != nil {
		return x, err
	}
	return x, Unauthorized
}

// Call with the user row locked, just like session revocation and credential changes.
func (s *Service) rememberedMFA(ctx context.Context, tx *sql.Tx, user string, proof BrowserProof) (bool, error) {
	if proof.Token == "" || proof.IP == "" {
		return false, nil
	}
	var ipHash string
	var expires time.Time
	err := tx.QueryRowContext(ctx, "SELECT ip_hash,expires_at FROM mfa_browser_verifications WHERE token_hash=? AND user_id=? FOR UPDATE", security.Hash(proof.Token), user).Scan(&ipHash, &expires)
	if err == sql.ErrNoRows {
		return false, nil
	}
	if err != nil {
		return false, err
	}
	if !security.Equal(ipHash, security.Hash(proof.IP)) || !time.Now().Before(expires) {
		return false, nil
	}
	_, err = tx.ExecContext(ctx, "UPDATE mfa_browser_verifications SET last_seen_at=UTC_TIMESTAMP(6),fingerprint_hash=?,device_name=? WHERE token_hash=?", security.Hash(proof.UserAgent), BrowserName(proof.UserAgent), security.Hash(proof.Token))
	return err == nil, err
}

func rememberedBrowserHash(remembered bool, token string) any {
	if !remembered {
		return nil
	}
	return security.Hash(token)
}
func recentAuthentication(remembered bool) any {
	if !remembered {
		return nil
	}
	return time.Now().UTC()
}

func sessionFamily(x Session) string {
	if x.FamilyHash != "" {
		return x.FamilyHash
	}
	return x.Hash
}
