package core

import (
	"context"
	"database/sql"
	"time"
)

const authLimitWindow = 15 * time.Minute

type AuthCleanupResult struct {
	Sessions             int64
	AuthLimits           int64
	BrowserVerifications int64
}

// CleanupExpiredAuthState removes authentication state that can no longer be
// accepted by Session or Limit. It is safe to run repeatedly from the worker.
func (s *Service) CleanupExpiredAuthState(ctx context.Context) (AuthCleanupResult, error) {
	now := time.Now().UTC().Truncate(time.Microsecond)
	idleCutoff := now.Add(-s.Config.Idle)
	limitCutoff := now.Add(-authLimitWindow)
	var cleaned AuthCleanupResult

	err := s.Transact(ctx, func(tx *sql.Tx) error {
		absolute, err := tx.ExecContext(ctx, "DELETE FROM sessions WHERE expires_at < ?", now)
		if err != nil {
			return err
		}
		n, err := absolute.RowsAffected()
		if err != nil {
			return err
		}
		cleaned.Sessions += n

		idle, err := tx.ExecContext(ctx, "DELETE FROM sessions WHERE last_activity < ?", idleCutoff)
		if err != nil {
			return err
		}
		n, err = idle.RowsAffected()
		if err != nil {
			return err
		}
		cleaned.Sessions += n

		proofs, err := tx.ExecContext(ctx, "DELETE FROM mfa_browser_verifications WHERE expires_at < ?", now)
		if err != nil {
			return err
		}
		cleaned.BrowserVerifications, err = proofs.RowsAffected()
		if err != nil {
			return err
		}
		limits, err := tx.ExecContext(ctx, "DELETE FROM auth_limits WHERE window_start < ?", limitCutoff)
		if err != nil {
			return err
		}
		cleaned.AuthLimits, err = limits.RowsAffected()
		return err
	})
	return cleaned, err
}
