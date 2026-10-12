package quotes

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func (s *Service) ExpenseReferenceInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (string, uint64, error) {
	q, e := s.access(ctx, tx, a, id, "cotizaciones.view", true, false)
	if e != nil {
		return "", 0, e
	}
	if q.Deleted != nil {
		return "", 0, core.Conflict
	}
	label := strings.TrimSpace(q.Number)
	if label == "" {
		label = strings.TrimSpace(q.Reference)
	}
	if label == "" {
		label = "Cotización #" + id
	}
	return label, q.Version, nil
}
