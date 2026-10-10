package quotes

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
	"strings"
	"time"
)

type TransitionInput struct {
	Status  string `json:"status"`
	Version uint64 `json:"version,string"`
	Reason  string `json:"reason,omitempty"`
}

var preliminaryFlow = []workflow.Edge{
	{From: "sent", To: "accepted"},
	{From: "sent", To: "rejected"},
	{From: "sent", To: "expired"},
	{From: "sent", To: "draft"},
	{From: "accepted", To: "draft"},
	{From: "rejected", To: "draft"},
	{From: "expired", To: "draft"},
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

func outcomeStatus(status string) bool {
	return status == "accepted" || status == "rejected" || status == "expired"
}

// Transition never fabricates approval or SMTP acceptance.
func (s *Service) Transition(ctx context.Context, a core.Actor, id, key string, p TransitionInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	p.Reason = strings.TrimSpace(p.Reason)
	if p.Version == 0 || (!editableStatus(p.Status) && !outcomeStatus(p.Status)) || !workflow.ReasonValid(p.Reason, outcomeStatus(p.Status)) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		var e error
		out, e = s.transitionInTransaction(ctx, tx, a, id, key, p)
		return e
	})
	return out, e
}

// Shared by the single-record action and durable bulk item transaction.
func (s *Service) transitionInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id, key string, p TransitionInput) (any, error) {
	if e := s.gate(ctx, tx, a, "cotizaciones.transition"); e != nil {
		return nil, e
	}
	q, e := s.access(ctx, tx, a, id, "cotizaciones.transition", true, false)
	if e != nil {
		return nil, e
	}
	if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
		return nil, e
	}
	return s.Core.Idempotent(ctx, tx, a, "quotes.transition:"+id, key, p, func() (any, error) {
		if q.Deleted != nil || q.Version != p.Version || !workflow.Allows(preliminaryFlow, q.Status, p.Status) {
			return nil, core.Conflict
		}
		if (q.Status == "sent" || outcomeStatus(q.Status)) && !workflow.ReasonValid(p.Reason, true) {
			return nil, core.Invalid
		}
		if p.Status == "expired" {
			date, e := time.Parse("2006-01-02", q.Date)
			if e != nil {
				return nil, e
			}
			today := time.Now().In(time.FixedZone("Honduras", -6*60*60)).Format("2006-01-02")
			if today <= date.AddDate(0, 0, q.ValidityDays).Format("2006-01-02") {
				return nil, core.Conflict
			}
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
		if p.Reason != "" {
			if e := s.Core.Audit(ctx, tx, a, "quote.transition_reason", "quotes", id, "success", map[string]string{"from": q.Status, "to": p.Status, "reason": p.Reason}); e != nil {
				return nil, e
			}
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
}

