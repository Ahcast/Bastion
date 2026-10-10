package quotes

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

type TransitionInput struct {
	Status  string `json:"status"`
	Version uint64 `json:"version,string"`
}

var preliminaryFlow = []workflow.Edge{
	{From: "approved", To: "draft"},
	{From: "draft", To: "costing"},
	{From: "costing", To: "draft"},
	{From: "costing", To: "review"},
	{From: "review", To: "draft"},
	{From: "review", To: "costing"},
}

func editableStatus(status string) bool {
	return status == "draft" || status == "costing" || status == "review"
}

// Transition changes only a saved revision; approval and emission are separate
// actions and cannot be reached through this preliminary graph.
func (s *Service) Transition(ctx context.Context, a core.Actor, id, key string, p TransitionInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if p.Version == 0 || !editableStatus(p.Status) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.transition"); e != nil {
			return e
		}
		q, e := s.access(ctx, tx, a, id, "cotizaciones.transition", true, false)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.transition:"+id, key, p, func() (any, error) {
			if q.Deleted != nil || q.Version != p.Version || !workflow.Allows(preliminaryFlow, q.Status, p.Status) {
				return nil, core.Conflict
			}
			if e := s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			if p.Status == "review" && len(q.Items) == 0 {
				return nil, core.Invalid
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET status=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, p.Status, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			q.Status = p.Status
			q.Version++
			v, e := s.record(ctx, tx, a, q, "quote.status_changed")
			if e != nil {
				return nil, e
			}
			result := v.(map[string]string)
			result["status"] = q.Status
			return result, nil
		})
		return e
	})
	return out, e
}
