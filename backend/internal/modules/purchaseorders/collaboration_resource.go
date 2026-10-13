package purchaseorders

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
	var deleted sql.NullTime
	err = q.QueryRowContext(ctx, `SELECT CAST(created_by AS CHAR),COALESCE(CAST(team_id AS CHAR),''),order_number,version,deleted_at FROM customer_purchase_orders WHERE id=?`, id).Scan(&owner, &team, &label, &version, &deleted)
	if err == sql.ErrNoRows || deleted.Valid {
		err = core.Missing
		return
	}
	if err != nil {
		return
	}
	label = strings.TrimSpace(label)
	if label == "" {
		label = "Orden de compra #" + id
	}
	return
}
