package processes

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
)

type QuoteSource struct {
	ID        string
	Entity    string
	Reference string
	Version   uint64
}

// QuoteSourceInTransaction exposes only the authorized conversion fields. It
// intentionally omits source specifications, documents and personal notes.
func (s *Service) QuoteSourceInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (QuoteSource, error) {
	if core.ID(id) != nil {
		return QuoteSource{}, core.Invalid
	}
	var p QuoteSource
	var owner string
	var team sql.NullString
	e := tx.QueryRowContext(ctx, `SELECT id,entity,reference,version,created_by,team_id FROM processes WHERE id=? FOR UPDATE`, id).Scan(&p.ID, &p.Entity, &p.Reference, &p.Version, &owner, &team)
	if e == sql.ErrNoRows {
		return p, core.Missing
	}
	if e != nil {
		return p, e
	}
	if e = s.Core.Authorize(ctx, tx, a, "procesos.view", owner, team.String, false); e == core.Forbidden {
		return p, core.Missing
	}
	return p, e
}
