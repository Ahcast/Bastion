package invoices

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func (s *Service) CollaborationResource(ctx context.Context, q core.Querier, id string) (owner, team, label string, version uint64, err error) {
	if core.ID(id) != nil {
		err = core.Missing
		return
	}
	var number sql.NullString
	var status string
	err = q.QueryRowContext(ctx, `SELECT CAST(created_by AS CHAR),COALESCE(CAST(team_id AS CHAR),''),invoice_number,status,version FROM invoices WHERE id=?`, id).Scan(&owner, &team, &number, &status, &version)
	if err == sql.ErrNoRows || status == "cancelled" {
		err = core.Missing
		return
	}
	if err != nil {
		return
	}
	label = strings.TrimSpace(number.String)
	if label == "" {
		label = "Factura #" + id
	}
	return
}
