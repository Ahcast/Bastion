package collaboration

import (
	"context"
	"database/sql"
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

type LinkInput struct {
	ResourceType    string `json:"resource_type"`
	ResourceID      string `json:"resource_id"`
	ResourceVersion uint64 `json:"resource_version,string"`
}

type SetLinksInput struct {
	Version uint64      `json:"version,string"`
	Links   []LinkInput `json:"links"`
}

type Link struct {
	ResourceType    string    `json:"resource_type"`
	ResourceID      string    `json:"resource_id"`
	ResourceVersion uint64    `json:"resource_version,string"`
	CurrentVersion  uint64    `json:"current_version,string"`
	Label           string    `json:"label"`
	CurrentLabel    string    `json:"current_label"`
	Stale           bool      `json:"stale"`
	LinkedBy        string    `json:"linked_by"`
	LinkedAt        time.Time `json:"linked_at"`
}

func linkTypeAllowed(v string) bool {
	return v == "processes" || v == "quotes" || v == "purchase_orders" || v == "invoices"
}

func validateLinksInput(p SetLinksInput) error {
	if p.Version == 0 || p.Version == ^uint64(0) || len(p.Links) > 20 {
		return core.Invalid
	}
	seen := map[string]bool{}
	for _, l := range p.Links {
		if !linkTypeAllowed(l.ResourceType) || core.ID(l.ResourceID) != nil || l.ResourceVersion == 0 || l.ResourceVersion == ^uint64(0) {
			return core.Invalid
		}
		n, _ := strconv.ParseUint(l.ResourceID, 10, 64)
		key := l.ResourceType + ":" + strconv.FormatUint(n, 10)
		if seen[key] {
			return core.Invalid
		}
		seen[key] = true
	}
	return nil
}

func (s *Service) Links(ctx context.Context, a core.Actor, entryID string) ([]Link, error) {
	if core.ID(entryID) != nil {
		return nil, core.Invalid
	}
	entry, e := s.entry(ctx, s.Core.DB, a, entryID, "colaboracion.view")
	if e != nil {
		return nil, e
	}
	if _, e = s.Core.DocumentMetadata(ctx, s.Core.DB, a, entry.DocumentID); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT resource_type,CAST(resource_id AS CHAR),resource_version,label_snapshot,CAST(linked_by AS CHAR),linked_at FROM collaboration_links WHERE entry_id=? ORDER BY resource_type,resource_id`, entryID)
	if e != nil {
		return nil, e
	}
	candidates := []Link{}
	for rows.Next() {
		var item Link
		if e = rows.Scan(&item.ResourceType, &item.ResourceID, &item.ResourceVersion, &item.Label, &item.LinkedBy, &item.LinkedAt); e != nil {
			rows.Close()
			return nil, e
		}
		candidates = append(candidates, item)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return nil, e
	}
	out := make([]Link, 0, len(candidates))
	for _, item := range candidates {
		r, resourceErr := s.resource(ctx, s.Core.DB, a, item.ResourceType, item.ResourceID)
		if resourceErr == core.Missing || resourceErr == core.Forbidden {
			continue
		}
		if resourceErr != nil {
			return nil, resourceErr
		}
		item.CurrentVersion, item.CurrentLabel = r.Version, r.Label
		item.Stale = r.Version != item.ResourceVersion || strings.TrimSpace(r.Label) != strings.TrimSpace(item.Label)
		out = append(out, item)
	}
	return out, nil
}

func (s *Service) SetLinks(ctx context.Context, a core.Actor, entryID, key string, p SetLinksInput) (any, error) {
	if a.Type != "user" || core.ID(entryID) != nil || key == "" || len(key) > 100 || validateLinksInput(p) != nil {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		entry, e := s.entry(ctx, tx, a, entryID, "colaboracion.link")
		if e != nil {
			return e
		}
		if _, e = s.Core.DocumentMetadata(ctx, tx, a, entry.DocumentID); e != nil {
			return e
		}
		owner := entry.UploadedBy
		if entry.Scope == "person" {
			owner = entry.PersonID
		}
		if e = s.gate(ctx, tx, a, "colaboracion.link", owner, entry.TeamID, true); e != nil {
			return e
		}
		type captured struct {
			input LinkInput
			label string
		}
		capturedLinks := make([]captured, 0, len(p.Links))
		for _, input := range p.Links {
			r, e := s.resource(ctx, tx, a, input.ResourceType, input.ResourceID)
			if e != nil {
				return e
			}
			if r.Version != input.ResourceVersion {
				return core.Conflict
			}
			capturedLinks = append(capturedLinks, captured{input: input, label: r.Label})
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.links:"+entryID, key, p, func() (any, error) {
			var current uint64
			if e := tx.QueryRowContext(ctx, `SELECT version FROM collaboration_entries WHERE id=? FOR UPDATE`, entryID).Scan(&current); e != nil {
				if e == sql.ErrNoRows {
					return nil, core.Missing
				}
				return nil, e
			}
			if current != p.Version || entry.Version != p.Version {
				return nil, core.Conflict
			}
			if _, e := tx.ExecContext(ctx, `DELETE FROM collaboration_links WHERE entry_id=?`, entryID); e != nil {
				return nil, e
			}
			for _, link := range capturedLinks {
				if _, e := tx.ExecContext(ctx, `INSERT INTO collaboration_links(entry_id,resource_type,resource_id,resource_version,label_snapshot,linked_by,linked_at) VALUES(?,?,?,?,?,?,UTC_TIMESTAMP(6))`, entryID, link.input.ResourceType, link.input.ResourceID, link.input.ResourceVersion, link.label, a.UserID); e != nil {
					return nil, e
				}
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_entries SET version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, entryID, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.links_updated", "collaboration_entries", entryID, "success", map[string]any{"link_count": len(capturedLinks), "version": p.Version + 1}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.links_updated", "collaboration_entries", entryID, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": entryID, "version": fmt.Sprint(p.Version + 1)}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) PendingApprovals(ctx context.Context, a core.Actor, page, perPage int) (workflow.InboxPage, error) {
	out := workflow.InboxPage{Data: []workflow.InboxItem{}, Page: page, PerPage: perPage}
	if !pageBounds(page, perPage) {
		return out, core.Invalid
	}
	if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.view"); e != nil {
		return out, e
	}
	if s.Approvals == nil {
		return out, core.Disabled
	}
	return s.Approvals.PendingInbox(ctx, a, page, perPage, func(ctx context.Context, actor core.Actor, item workflow.InboxItem) error {
		_, e := s.resource(ctx, s.Core.DB, actor, item.EntityType, item.EntityID)
		return e
	})
}
