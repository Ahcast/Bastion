package invoices

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func (s *Service) ExpenseReferenceInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (string, uint64, error) {
	o, e := s.access(ctx, tx, a, id, "facturas.view", true)
	if e != nil {
		return "", 0, e
	}
	if o.CancelledAt != nil || o.Status == "cancelled" {
		return "", 0, core.Conflict
	}
	label := ""
	if o.Number != nil {
		label = strings.TrimSpace(*o.Number)
	}
	if label == "" {
		label = "Factura #" + id
	}
	return label, o.Version, nil
}
