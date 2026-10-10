package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
	"strings"
	"time"
)

type ApprovalRequest struct {
	QuoteVersion uint64 `json:"quote_version,string"`
	Expires      string `json:"expires_at"`
	Reason       string `json:"reason"`
}
type ApprovalDecision struct {
	QuoteVersion uint64 `json:"quote_version,string"`
	Version      uint64 `json:"version,string"`
	Decision     string `json:"decision"`
	Reason       string `json:"reason"`
}
type ApprovalRevoke struct {
	QuoteVersion uint64 `json:"quote_version,string"`
	Version      uint64 `json:"version,string"`
	Reason       string `json:"reason"`
}

// ApprovalBinding builds the complete internal input fingerprint. A caller must
// lock/load the aggregate and its items, and authorize the proposed operation.
func (s *Service) ApprovalBinding(ctx context.Context, tx *sql.Tx, a core.Actor, q Quote) (workflow.Binding, error) {
	var b workflow.Binding
	if q.Deleted != nil || len(q.Items) == 0 {
		return b, core.Conflict
	}
	company, cv, e := s.Core.PublicCompany(ctx, tx)
	if e != nil {
		return b, e
	}
	attachments, e := s.Core.EntityFileEvidence(ctx, tx, a, "quotes", q.ID)
	if e != nil {
		return b, e
	}
	input := struct {
		Schema      int                 `json:"schema"`
		Quote       Quote               `json:"quote"`
		Public      PublicQuote         `json:"public"`
		Attachments []core.FileEvidence `json:"attachments"`
	}{1, q, Public(q, company, cv, s.Logo, s.Stamp), attachments}
	raw, e := json.Marshal(input)
	if e != nil {
		return b, e
	}
	if len(raw) > 4*1024*1024 {
		return b, core.Invalid
	}
	return workflow.Binding{Action: "quote.emit", Entity: "quotes", ID: q.ID, Version: q.Version, Hash: security.Hash(string(raw)), Owner: q.Owner, Team: q.Team, ViewPermission: "cotizaciones.view", RequestPermission: "cotizaciones.request_approval", DecisionPermission: "cotizaciones.approve", RevokePermission: "cotizaciones.revoke_approval"}, nil
}
func (s *Service) RequestApproval(ctx context.Context, a core.Actor, id, key string, p ApprovalRequest) (any, error) {
	if s.Approvals == nil {
		return nil, core.Disabled
	}
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	expires, e := time.Parse(time.RFC3339Nano, p.Expires)
	p.Reason = strings.TrimSpace(p.Reason)
	if e != nil || p.QuoteVersion == 0 || expires.Year() < 1000 || expires.Year() > 9999 || !workflow.ReasonValid(p.Reason, false) {
		return nil, core.Invalid
	}
	expires = expires.UTC().Truncate(time.Microsecond)
	p.Expires = expires.Format(time.RFC3339Nano)
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.request_approval"); e != nil {
			return e
		}
		q, e := s.access(ctx, tx, a, id, "cotizaciones.request_approval", true, false)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.approval_request:"+id, key, p, func() (any, error) {
			// A committed request remains replayable after its deadline; only a new
			// intention must have future validity. Replay still checks live access.
			if !expires.After(time.Now()) {
				return nil, core.Invalid
			}
			if q.Status != "review" || q.Deleted != nil || q.Version != p.QuoteVersion || q.Version == ^uint64(0) {
				return nil, core.Conflict
			}
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			q.Status = "approved"
			q.Version++
			b, e := s.ApprovalBinding(ctx, tx, a, q)
			if e != nil {
				return nil, e
			}
			return s.Approvals.Request(ctx, tx, a, b, expires, p.Reason)
		})
		return e
	})
	return out, e
}
func (s *Service) DecideApproval(ctx context.Context, a core.Actor, id, approval, key string, p ApprovalDecision) (any, error) {
	if s.Approvals == nil {
		return nil, core.Disabled
	}
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	p.Reason = strings.TrimSpace(p.Reason)
	if p.Version == 0 || p.QuoteVersion == 0 || !workflow.ReasonValid(p.Reason, true) || (p.Decision != "approved" && p.Decision != "rejected") {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.approve"); e != nil {
			return e
		}
		q, e := s.access(ctx, tx, a, id, "cotizaciones.approve", true, true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
			return e
		}
		v, e := s.Approvals.Get(ctx, tx, approval, "quotes", id)
		if e != nil {
			return e
		}
		// These checks run before cached results as well as before a new decision.
		if a.UserID == v.RequestedBy || a.UserID == q.Owner {
			return core.Forbidden
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.approval_decide:"+approval, key, p, func() (any, error) {
			if q.Deleted != nil || q.Status != "review" || q.Version != p.QuoteVersion || q.Version == ^uint64(0) {
				return nil, core.Conflict
			}
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			q.Status = "approved"
			q.Version++
			b, e := s.ApprovalBinding(ctx, tx, a, q)
			if e != nil {
				return nil, e
			}
			if _, e = s.Approvals.Decide(ctx, tx, a, v, b, p.Version, p.Decision, p.Reason); e != nil {
				return nil, e
			}
			if p.Decision == "rejected" {
				q.Status = "review"
			}
			return s.approvalRevision(ctx, tx, a, q, approval, "quote."+p.Decision)
		})
		return e
	})
	return out, e
}
func (s *Service) RevokeApproval(ctx context.Context, a core.Actor, id, approval, key string, p ApprovalRevoke) (any, error) {
	if s.Approvals == nil {
		return nil, core.Disabled
	}
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	p.Reason = strings.TrimSpace(p.Reason)
	if p.Version == 0 || p.QuoteVersion == 0 || !workflow.ReasonValid(p.Reason, true) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.revoke_approval"); e != nil {
			return e
		}
		q, e := s.access(ctx, tx, a, id, "cotizaciones.revoke_approval", true, true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", q.Owner, q.Team, false); e != nil {
			return e
		}
		v, e := s.Approvals.Get(ctx, tx, approval, "quotes", id)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.approval_revoke:"+approval, key, p, func() (any, error) {
			if q.Deleted != nil || q.Version != p.QuoteVersion || (q.Status != "review" && q.Status != "approved") || q.Version == ^uint64(0) {
				return nil, core.Conflict
			}
			expected := q.Version
			if v.Status == "pending" {
				expected++
			}
			if expected != v.Binding.Version {
				return nil, core.Conflict
			}
			if _, e = s.Approvals.Revoke(ctx, tx, a, v, p.Version, p.Reason); e != nil {
				return nil, e
			}
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			q.Status = "review"
			q.Version++
			return s.approvalRevision(ctx, tx, a, q, approval, "quote.approval_revoked")
		})
		return e
	})
	return out, e
}
func (s *Service) approvalRevision(ctx context.Context, tx *sql.Tx, a core.Actor, q Quote, approval, topic string) (any, error) {
	if e := core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET status=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, q.Status, a.UserID, q.ID, q.Version-1)); e != nil {
		return nil, e
	}
	v, e := s.record(ctx, tx, a, q, topic)
	if e != nil {
		return nil, e
	}
	m := v.(map[string]string)
	m["status"] = q.Status
	m["approval_id"] = approval
	return m, nil
}

type ApprovalView struct {
	workflow.Approval
	Valid         bool   `json:"valid"`
	InvalidReason string `json:"invalid_reason"`
}

func (s *Service) ApprovalList(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if s.Approvals == nil {
		return nil, core.Disabled
	}
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	data := []ApprovalView{}
	var total int
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.access(ctx, tx, a, id, "cotizaciones.view", true, false)
		if e != nil {
			return e
		}
		if q.Deleted != nil {
			return core.Missing
		}
		approvals, count, e := s.Approvals.List(ctx, tx, a, workflow.Binding{Entity: "quotes", ID: id, Owner: q.Owner, Team: q.Team, ViewPermission: "cotizaciones.view"}, page, limit)
		if e != nil {
			return e
		}
		total = count
		var binding *workflow.Binding
		for _, v := range approvals {
			view := ApprovalView{Approval: v, InvalidReason: "approval_not_active"}
			if v.Status == "approved" {
				if q.Status != "approved" || v.Binding.Version != q.Version {
					view.InvalidReason = "approval_changed"
				} else {
					if binding == nil {
						if e = s.items(ctx, tx, &q); e != nil {
							return e
						}
						b, e := s.ApprovalBinding(ctx, tx, a, q)
						if e != nil {
							return e
						}
						binding = &b
					}
					e = s.Approvals.Verify(ctx, tx, v, *binding)
					view.Valid = e == nil
					view.InvalidReason = ""
					if e != nil {
						if ce, ok := e.(*core.Error); ok {
							view.InvalidReason = ce.Code
						} else {
							return e
						}
					}
				}
			}
			data = append(data, view)
		}
		return nil
	})
	return map[string]any{"data": data, "total": total, "page": page, "per_page": limit}, e
}

// VerifyApprovalInTransaction is consumed by a future emission service; no
// HTTP route in this unit can turn approval into delivery or presentation.
func (s *Service) VerifyApprovalInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id, approval string) (workflow.Binding, error) {
	var b workflow.Binding
	if s.Approvals == nil {
		return b, core.Disabled
	}
	q, e := s.access(ctx, tx, a, id, "cotizaciones.view", true, false)
	if e != nil {
		return b, e
	}
	if q.Status != "approved" || q.Deleted != nil {
		return b, core.Conflict
	}
	if e = s.items(ctx, tx, &q); e != nil {
		return b, e
	}
	b, e = s.ApprovalBinding(ctx, tx, a, q)
	if e != nil {
		return b, e
	}
	v, e := s.Approvals.Get(ctx, tx, approval, "quotes", id)
	if e != nil {
		return b, e
	}
	return b, s.Approvals.Verify(ctx, tx, v, b)
}
