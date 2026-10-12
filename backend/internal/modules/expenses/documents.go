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
	o, e := s.access(ctx, q, a, id, permission, write)
	if e != nil {
		return e
	}
	if e = s.Core.Authorize(ctx, q, a, "gastos.view", o.CreatedBy, o.TeamID, false); e != nil {
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
		if e = s.Core.Authorize(ctx, tx, a, "gastos.view", o.CreatedBy, o.TeamID, false); e != nil {
			return e
		}

		if o.DeletedAt != nil {
			return core.Conflict
		}
		if e = s.Core.Authorize(ctx, tx, a, "gastos.documents.write", o.CreatedBy, o.TeamID, true); e != nil {
			return e
		}
		binding, e := s.Core.DocumentBindingAccess(ctx, tx, a, p.DocumentID, true, "expenses", o.ID)
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
			if e := s.Core.BindDocument(ctx, tx, a, p.DocumentID, "expenses", o.ID, p.DocumentVersion); e != nil {
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
			return map[string]string{"id": o.ID, "version": fmt.Sprint(p.Version + 1), "document_id": p.DocumentID, "document_version": fmt.Sprint(p.DocumentVersion + 1)}, nil
		})
		return e
	})
	return out, dbError(e)
}

func (s *Service) Documents(ctx context.Context, a core.Actor, id string, page, perPage int, deletedFilter ...bool) (DocumentPage, error) {
	out := DocumentPage{Data: []ExpenseDocument{}, Page: page, PerPage: perPage}
	deleted := len(deletedFilter) > 0 && deletedFilter[0]
	owner, e := s.Get(ctx, a, id)
	if e != nil {
		return out, e
	}
	id = owner.ID
	meta, e := s.Core.EntityDocumentMetadataPage(ctx, s.Core.DB, a, "expenses", id, page, perPage, deleted)
	if e != nil {
		return out, e
	}
	out.Total = meta.Total
	for _, m := range meta.Data {
		var v ExpenseDocument
		v.DocumentMetadata = m
		if e = s.Core.DB.QueryRowContext(ctx, `SELECT document_version,CAST(linked_by AS CHAR),linked_at FROM expense_documents WHERE expense_id=? AND document_id=?`, id, m.ID).Scan(&v.LinkedDocumentVersion, &v.LinkedBy, &v.LinkedAt); e != nil {
			return out, e
		}
		out.Data = append(out.Data, v)
	}
	return out, nil
}
