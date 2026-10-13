package core

import (
	"context"
	"database/sql"
)

// protectSystemAdministrator keeps the bootstrap administrator (role 1)
// immutable through user-management recovery paths. Recovery for this account
// must use the trusted administrative/CLI procedure rather than a delegated
// core.users.manage permission.
func (s *Service) protectSystemAdministrator(ctx context.Context, tx *sql.Tx, userID string) error {
	var protected int
	if err := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM user_roles WHERE user_id=? AND role_id=1", userID).Scan(&protected); err != nil {
		return err
	}
	if protected > 0 {
		return Forbidden
	}
	return nil
}
