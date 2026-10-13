package processes

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

// CollaborationResource exposes only the owner's authorization context and a
// stable display snapshot. Collaboration never reads the processes table.
func (s *Service) CollaborationResource(ctx context.Context, q core.Querier, id string) (owner, team, label string, version uint64, err error) {
	if core.ID(id) != nil {
		err = core.Missing
		return
	}
	var reference, description string
	err = q.QueryRowContext(ctx, `SELECT CAST(created_by AS CHAR),COALESCE(CAST(team_id AS CHAR),''),reference,description,version FROM processes WHERE id=?`, id).Scan(&owner, &team, &reference, &description, &version)
	if err == sql.ErrNoRows {
		err = core.Missing
		return
	}
	if err != nil {
		return
	}
	label = strings.TrimSpace(reference)
	if label == "" {
		label = strings.TrimSpace(description)
	}
	if label == "" {
		label = "Proceso #" + id
	}
	if len([]rune(label)) > 1000 {
		label = string([]rune(label)[:1000])
	}
	return
}
