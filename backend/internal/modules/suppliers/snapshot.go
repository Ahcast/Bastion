package suppliers

import (
	"context"
	"database/sql"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

// SnapshotInTransaction allows a caller-owned aggregate transaction to resolve
// an authorized active supplier snapshot without nested transactions or direct
// cross-module SQL access.
func (s *Service) SnapshotInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (Snapshot, error) {
	if e := s.gate(ctx, tx, a, "proveedores.view"); e != nil {
		return Snapshot{}, e
	}
	v, e := s.access(ctx, tx, a, id, "proveedores.view", true, false)
	if e != nil {
		return Snapshot{}, e
	}
	if !v.Active {
		return Snapshot{}, core.Conflict
	}
	idCopy := v.ID
	return Snapshot{
		ID:      &idCopy,
		Name:    v.Name,
		RTN:     v.RTN,
		Contact: v.ContactName,
		Phone:   v.Phone,
		Email:   v.Email,
		Address: v.Address,
		Country: v.Country,
		Website: v.Website,
		Version: v.Version,
	}, nil
}
