package customers

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
)

// SnapshotInTransaction allows a caller-owned aggregate transaction to resolve
// authorized snapshots without nested transactions or cross-module SQL writes.
func (s *Service) SnapshotInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (Snapshot, error) {
	if e := s.gate(ctx, tx, a, "clientes.view"); e != nil {
		return Snapshot{}, e
	}
	c, e := s.access(ctx, tx, a, id, "clientes.view", true, false)
	if e != nil {
		return Snapshot{}, e
	}
	if !c.Active {
		return Snapshot{}, core.Conflict
	}
	return Snapshot{c.ID, c.Name, c.RTN, c.ContactName, c.Phone, c.Email, c.Address, c.Version}, nil
}
func (s *Service) ResolveInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, name string) (*Snapshot, error) {
	// A crawler entity can be longer than a customer name and therefore cannot
	// match that catalogue. Lack of view grant must not disclose its existence.
	if e := s.gate(ctx, tx, a, "clientes.view"); e != nil {
		return nil, e
	}
	if !text(name, 180, true) {
		return nil, nil
	}
	where, args := scope(a, "clientes.view")
	where += ` AND c.is_active=TRUE AND c.canonical_name=?`
	args = append(args, CanonicalName(name))
	var id string
	e := tx.QueryRowContext(ctx, `SELECT c.id FROM customers c WHERE `+where, args...).Scan(&id)
	if e == sql.ErrNoRows {
		return nil, nil
	}
	if e != nil {
		return nil, e
	}
	v, e := s.SnapshotInTransaction(ctx, tx, a, id)
	return &v, e
}
