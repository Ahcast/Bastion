package expenses

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type LinkDocumentInput struct {
	Version         uint64 `json:"version,string"`
	DocumentID      string `json:"document_id"`
	DocumentVersion uint64 `json:"document_version,string"`
}

type ExpenseDocument struct {
	core.DocumentMetadata
	LinkedDocumentVersion uint64    `json:"linked_document_version,string"`
	LinkedBy              string    `json:"linked_by"`
	LinkedAt              time.Time `json:"linked_at"`
}

type DocumentPage struct {
	Data    []ExpenseDocument `json:"data"`
	Total   int               `json:"total"`
	Page    int               `json:"page"`
	PerPage int               `json:"per_page"`
}

func (s *Service) DocumentPolicy(ctx context.Context, q core.Querier, a core.Actor, id string, write bool) error {
	permission := "gastos.view"
	if write {
		permission = "gastos.documents.write"
	}
	o, e := s.access(ctx, q, a, id, permission, false)
	if e != nil {
		return e
	}
	if o.DeletedAt != nil {
		return core.Missing
	}
	if write {
		return s.Core.Authorize(ctx, q, a, permission, o.CreatedBy, o.TeamID, true)
	}
	return nil
}

func (s *Service) LinkDocument(ctx context.Context, a core.Actor, id, key string, p LinkDocumentInput) (any, error) {
	if a.Type != "user" || core.ID(id) != nil || key == "" || p.Version == 0 || p.Version == ^uint64(0) || core.ID(p.DocumentID) != nil || p.DocumentVersion == 0 || p.DocumentVersion == ^uint64(0) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "gastos.documents.write"); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "gastos.documents.write", true)
		if e != nil {
			return e
		}
		if o.DeletedAt != nil {
			return core.Conflict
		}
		if e = s.Core.Authorize(ctx, tx, a, "gastos.documents.write", o.CreatedBy, o.TeamID, true); e != nil {
			return e
		}
		binding, e := s.Core.DocumentBindingAccess(ctx, tx, a, p.DocumentID, true)
		if e != nil {
			return e
		}
		if binding.Generated || (binding.EntityType != "" && (binding.EntityType != "expenses" || binding.EntityID != id)) {
			return core.Conflict
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "expenses.document:"+id+":"+p.DocumentID, key, p, func() (any, error) {
			if o.Version != p.Version || binding.Version != p.DocumentVersion || binding.EntityType != "" {
				return nil, core.Conflict
			}
			if e := s.Core.BindDocument(ctx, tx, a, p.DocumentID, "expenses", id, p.DocumentVersion); e != nil {
				return nil, e
			}
			if _, e := tx.ExecContext(ctx, `INSERT INTO expense_documents(expense_id,document_id,document_version,linked_by,linked_at) VALUES(?,?,?,?,UTC_TIMESTAMP(6))`, id, p.DocumentID, p.DocumentVersion+1, a.UserID); e != nil {
				return nil, e
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE expenses SET version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND deleted_at IS NULL`, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.evidence(ctx, tx, a, "expense.document_linked", id, p.Version+1, ""); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1), "document_id": p.DocumentID, "document_version": fmt.Sprint(p.DocumentVersion + 1)}, nil
		})
		return e
	})
	return out, dbError(e)
}

func (s *Service) Documents(ctx context.Context, a core.Actor, id string, page, perPage int) (DocumentPage, error) {
	out := DocumentPage{Data: []ExpenseDocument{}, Page: page, PerPage: perPage}
	if core.ID(id) != nil || page < 1 || page > 100000 || perPage < 1 || perPage > 100 {
		return out, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "gastos.view"); e != nil {
		return out, e
	}
	o, e := s.access(ctx, s.Core.DB, a, id, "gastos.view", false)
	if e != nil {
		return out, e
	}
	if o.DeletedAt != nil {
		return out, core.Missing
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(document_id AS CHAR),document_version,CAST(linked_by AS CHAR),linked_at FROM expense_documents WHERE expense_id=? ORDER BY linked_at DESC,document_id DESC`, id)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	visible := []ExpenseDocument{}
	for rows.Next() {
		var documentID, linkedBy string
		var linkedVersion uint64
		var linkedAt time.Time
		if e = rows.Scan(&documentID, &linkedVersion, &linkedBy, &linkedAt); e != nil {
			return out, e
		}
		meta, accessErr := s.Core.DocumentMetadata(ctx, s.Core.DB, a, documentID)
		if accessErr == core.Missing || accessErr == core.Forbidden {
			continue
		}
		if accessErr != nil {
			return out, accessErr
		}
		visible = append(visible, ExpenseDocument{DocumentMetadata: meta, LinkedDocumentVersion: linkedVersion, LinkedBy: linkedBy, LinkedAt: linkedAt})
	}
	if e = rows.Err(); e != nil {
		return out, e
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
