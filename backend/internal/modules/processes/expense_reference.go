package processes

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

// ExpenseReferenceInTransaction locks and authorizes the process before a
// documentary expense reference is captured. Expenses never reads this table.
func (s *Service) ExpenseReferenceInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (string, uint64, error) {
	if core.ID(id) != nil {
		return "", 0, core.Invalid
	}
	var reference, description, owner, team string
	var version uint64
	if e := tx.QueryRowContext(ctx, `SELECT reference,description,version,CAST(created_by AS CHAR),COALESCE(CAST(team_id AS CHAR),'') FROM processes WHERE id=? FOR UPDATE`, id).Scan(&reference, &description, &version, &owner, &team); e != nil {
		if e == sql.ErrNoRows {
			return "", 0, core.Missing
		}
		return "", 0, e
	}
	if e := s.Core.Authorize(ctx, tx, a, "procesos.view", owner, team, false); e != nil {
		if e == core.Forbidden {
			return "", 0, core.Missing
		}
		return "", 0, e
	}
	label := strings.TrimSpace(reference)
	if label == "" {
		label = strings.TrimSpace(description)
	}
	if label == "" {
		label = "Proceso #" + id
	}
	if len([]rune(label)) > 1000 {
		label = string([]rune(label)[:1000])
	}
	return label, version, nil
}
