package core

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"
)

// PublicCompany returns only the nonsecret business header for derived documents.
func (s *Service) PublicCompany(ctx context.Context, q Querier) (CompanySettings, uint64, error) {
	var raw string
	var version uint64
	var c CompanySettings
	suffix := ""
	if _, ok := q.(*sql.Tx); ok {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT value,version FROM settings WHERE name='company'`+suffix).Scan(&raw, &version)
	if e == sql.ErrNoRows {
		return c, 0, Disabled
	}
	if e != nil {
		return c, 0, e
	}
	if json.Unmarshal([]byte(raw), &c) != nil || c.Name == "" {
		return c, 0, Invalid
	}
	return c, version, nil
}

// RegisterGeneratedFile is an internal API. Callers must have verified immutable
// storage of server-rendered bytes; uploaded user content still uses quarantine.
func (s *Service) RegisterGeneratedFile(ctx context.Context, tx *sql.Tx, a Actor, entity, eid, key string, f FileRequest) (string, error) {
	if !s.IntegrationEnabledUsing(ctx, tx, "files") {
		return "", Disabled
	}
	if ValidateFile(f) != nil || f.MediaType != "application/pdf" || len(key) > 200 {
		return "", Invalid
	}
	policy, ok := s.DocumentPolicies[entity]
	if !ok {
		return "", Forbidden
	}
	if e := policy(ctx, tx, a, eid, false); e != nil {
		return "", e
	}
	if e := s.Authorize(ctx, tx, a, "core.documents.write", a.UserID, "", false); e != nil {
		return "", e
	}
	r, e := tx.ExecContext(ctx, `INSERT INTO documents(owner_id,entity_type,entity_id,is_generated,created_at) VALUES(?,?,?,TRUE,UTC_TIMESTAMP(6))`, a.UserID, entity, eid)
	if e != nil {
		return "", e
	}
	n, e := r.LastInsertId()
	if e != nil {
		return "", e
	}
	id := WrapID(n)
	if _, e = tx.ExecContext(ctx, `INSERT INTO document_versions VALUES(?,1,?,?,?,?,?,'available','bastion-generated-pdf-v1',UTC_TIMESTAMP(6))`, id, key, f.Filename, f.MediaType, f.Size, f.Checksum); e != nil {
		return "", e
	}
	return id, s.Audit(ctx, tx, a, "files.generated", "document", id, "success", map[string]string{"entity_type": entity, "entity_id": eid})
}
func (s *Service) TrashFile(ctx context.Context, a Actor, id, key string, version uint64, restore bool) (any, error) {
	if a.Type != "user" {
		return nil, Forbidden
	}
	if ID(id) != nil || version == 0 {
		return nil, Invalid
	}
	var out any
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		state, e := s.lockDocumentWrite(ctx, tx, a, id, true)
		if e != nil {
			return e
		}
		if state.Entity.Valid {
			if policy, ok := s.DocumentTrashPolicies[state.Entity.String]; ok {
				if e = policy(ctx, tx, a, state.EntityID.String); e != nil {
					return e
				}
			}
		}
		old, deleted, generated := state.Version, state.Deleted, state.Generated

		out, e = s.Idempotent(ctx, tx, a, "files.trash:"+id, key, map[string]any{"version": version, "restore": restore}, func() (any, error) {
			if generated || version != old || restore != deleted.Valid {
				return nil, Conflict
			}
			var d any
			if !restore {
				d = time.Now().UTC()
			}
			if e := Changed(tx.ExecContext(ctx, `UPDATE documents SET deleted_at=?,version=version+1 WHERE id=? AND version=?`, d, id, version)); e != nil {
				return nil, e
			}
			if e := s.Audit(ctx, tx, a, "files.trash", "document", id, "success", map[string]bool{"restored": restore}); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(version + 1)}, nil
		})
		return e
	})
	return out, e
}
