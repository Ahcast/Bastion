package expenses

import (
	"context"
	"database/sql"
	"fmt"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
)

func validateReferences(p SetReferencesInput) (map[string]ReferenceInput, error) {
	if p.Version == 0 || p.Version == ^uint64(0) || p.References == nil || len(p.References) > len(referenceOrder) || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	out := make(map[string]ReferenceInput, len(p.References))
	for _, r := range p.References {
		validType := false
		for _, kind := range referenceOrder {
			if kind == r.EntityType {
				validType = true
				break
			}
		}
		if !validType || core.ID(r.EntityID) != nil || r.EntityVersion == 0 || r.EntityVersion == ^uint64(0) {
			return nil, core.Invalid
		}
		if _, exists := out[r.EntityType]; exists {
			return nil, core.Invalid
		}
		out[r.EntityType] = r
	}
	return out, nil
}

func (s *Service) References(ctx context.Context, a core.Actor, id string) ([]Reference, error) {
	if core.ID(id) != nil {
		return nil, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "gastos.view"); e != nil {
		return nil, e
	}
	if _, e := s.access(ctx, s.Core.DB, a, id, "gastos.view", false); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT entity_type,CAST(entity_id AS CHAR),entity_version,label_snapshot,CAST(linked_by AS CHAR),linked_at FROM expense_references WHERE expense_id=? ORDER BY FIELD(entity_type,'processes','quotes','purchase_orders','invoices')`, id)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Reference{}
	for rows.Next() {
		var r Reference
		if e = rows.Scan(&r.EntityType, &r.EntityID, &r.EntityVersion, &r.Label, &r.LinkedBy, &r.LinkedAt); e != nil {
			return nil, e
		}
		out = append(out, r)
	}
	return out, rows.Err()
}

func (s *Service) SetReferences(ctx context.Context, a core.Actor, id, key string, p SetReferencesInput) (any, error) {
	if a.Type != "user" || core.ID(id) != nil || key == "" {
		return nil, core.Invalid
	}
	requested, e := validateReferences(p)
	if e != nil {
		return nil, e
	}
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "gastos.references.write"); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "gastos.references.write", true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "gastos.view", o.CreatedBy, o.TeamID, false); e != nil {
			return e
		}

		if o.DeletedAt != nil {
			return core.Conflict
		}
		if e = s.Core.Authorize(ctx, tx, a, "gastos.references.write", o.CreatedBy, o.TeamID, true); e != nil {
			return e
		}

		type snapshot struct {
			input   ReferenceInput
			label   string
			version uint64
		}
		captured := make(map[string]snapshot, len(requested))
		sources := s.referenceSources()
		for _, kind := range referenceOrder {
			input, ok := requested[kind]
			if !ok {
				continue
			}
			source := referenceSource(sources, kind)
			if source == nil {
				return core.Disabled
			}
			label, version, e := source.ExpenseReferenceInTransaction(ctx, tx, a, input.EntityID)
			if e != nil {
				return e
			}
			captured[kind] = snapshot{input: input, label: label, version: version}
		}

		out, e = s.Core.Idempotent(ctx, tx, a, "expenses.references:"+id, key, p, func() (any, error) {
			if o.Version != p.Version {
				return nil, core.Conflict
			}
			for _, kind := range referenceOrder {
				if x, ok := captured[kind]; ok && x.version != x.input.EntityVersion {
					return nil, core.Conflict
				}
			}
			if _, e := tx.ExecContext(ctx, `DELETE FROM expense_references WHERE expense_id=?`, id); e != nil {
				return nil, e
			}
			for _, kind := range referenceOrder {
				x, ok := captured[kind]
				if !ok {
					continue
				}
				if _, e := tx.ExecContext(ctx, `INSERT INTO expense_references(expense_id,entity_type,entity_id,entity_version,label_snapshot,linked_by,linked_at) VALUES(?,?,?,?,?,?,UTC_TIMESTAMP(6))`, id, kind, x.input.EntityID, x.input.EntityVersion, x.label, a.UserID); e != nil {
					return nil, e
				}
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE expenses SET version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND deleted_at IS NULL`, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.evidence(ctx, tx, a, "expense.references_updated", id, p.Version+1, p.Reason); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1)}, nil
		})
		return e
	})
	return out, dbError(e)
}
