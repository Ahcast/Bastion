package purchaseorders

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func (s *Service) ExpenseReferenceInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (string, uint64, error) {
	o, e := s.access(ctx, tx, a, id, "ordenes.view", true)
	if e != nil {
		return "", 0, e
	}
	if o.Deleted != nil {
		return "", 0, core.Conflict
	}
	label := strings.TrimSpace(o.OrderNumber)
	if label == "" {
		label = "Orden de compra #" + id
	}
	return label, o.Version, nil
}
