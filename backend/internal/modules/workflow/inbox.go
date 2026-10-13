package workflow

import (
	"context"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type InboxItem struct {
	ID            string    `json:"id"`
	Action        string    `json:"action"`
	EntityType    string    `json:"entity_type"`
	EntityID      string    `json:"entity_id"`
	EntityVersion uint64    `json:"entity_version,string"`
	RequestedBy   string    `json:"requested_by"`
	Status        string    `json:"status"`
	Reason        string    `json:"reason"`
	ExpiresAt     time.Time `json:"expires_at"`
	Version       uint64    `json:"version,string"`
}

type InboxPage struct {
	Data    []InboxItem `json:"data"`
	Total   int         `json:"total"`
	Page    int         `json:"page"`
	PerPage int         `json:"per_page"`
}

// PendingInbox is a read-only projection. Workflow remains the only owner of
// approval decisions and Collaboration receives no decision capability.
type InboxVisibility func(context.Context, core.Actor, InboxItem) error

func (s *Service) PendingInbox(ctx context.Context, a core.Actor, page, perPage int, canReadResource InboxVisibility) (InboxPage, error) {
	out := InboxPage{Data: []InboxItem{}, Page: page, PerPage: perPage}
	if a.Type != "user" || page < 1 || page > 100000 || perPage < 1 || perPage > 100 {
		return out, core.Invalid
	}
	if canReadResource == nil {
		return out, core.Disabled
	}
	type candidate struct {
		InboxItem
		owner, team, viewPermission, decisionPermission string
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(id AS CHAR),action,entity_type,CAST(entity_id AS CHAR),entity_version,CAST(owner_id AS CHAR),team_id,view_permission,decision_permission,CAST(requested_by AS CHAR),status,reason,expires_at,version FROM approvals WHERE status='pending' AND expires_at>UTC_TIMESTAMP(6) ORDER BY expires_at,id`)
	if e != nil {
		return out, e
	}
	candidates := []candidate{}
	for rows.Next() {
		var c candidate
		if e = rows.Scan(&c.ID, &c.Action, &c.EntityType, &c.EntityID, &c.EntityVersion, &c.owner, &c.team, &c.viewPermission, &c.decisionPermission, &c.RequestedBy, &c.Status, &c.Reason, &c.ExpiresAt, &c.Version); e != nil {
			rows.Close()
			return out, e
		}
		candidates = append(candidates, c)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return out, e
	}
	visible := []InboxItem{}
	for _, c := range candidates {
		if c.RequestedBy == a.UserID || c.owner == a.UserID {
			continue
		}
		if e = s.Core.Authorize(ctx, s.Core.DB, a, c.viewPermission, c.owner, c.team, false); e != nil {
			if e == core.Forbidden {
				continue
			}
			return out, e
		}
		if e = s.Core.Authorize(ctx, s.Core.DB, a, c.decisionPermission, c.owner, c.team, false); e != nil {
			if e == core.Forbidden {
				continue
			}
			return out, e
		}
		if e = canReadResource(ctx, a, c.InboxItem); e != nil {
			if e == core.Missing || e == core.Forbidden {
				continue
			}
			return out, e
		}
		visible = append(visible, c.InboxItem)
	}
	out.Total = len(visible)
	start := (page - 1) * perPage
	if start >= len(visible) {
		return out, nil
	}
	end := start + perPage
	if end > len(visible) {
		end = len(visible)
	}
	out.Data = visible[start:end]
	return out, nil
}
