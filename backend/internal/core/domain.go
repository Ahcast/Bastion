package core

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

// EmitEvent records business facts in the same transaction as their aggregate.
// Events have no implicit job, email or AI consumer.
func (s *Service) EmitEvent(ctx context.Context, tx *sql.Tx, a Actor, topic, entity, id string, version uint64) error {
	_, e := tx.ExecContext(ctx, `INSERT INTO domain_events VALUES(?,?,1,?,?,?,?,?,UTC_TIMESTAMP(6))`, security.ID(), topic, entity, id, version, a.UserID, a.CorrelationID)
	return e
}

type DocumentPolicy func(context.Context, Querier, Actor, string, bool) error
type DocumentBindingPolicy func(context.Context, Querier, Actor, string) error
type DocumentVersionPolicy func(context.Context, Querier, Actor, string) error
type DocumentTrashPolicy func(context.Context, Querier, Actor, string) error

type DocumentBinding struct {
	OwnerID    string
	TeamID     string
	Version    uint64
	EntityType string
	EntityID   string
	Generated  bool
}

func (s *Service) authorizeDocumentVersion(ctx context.Context, q Querier, a Actor, entity, entityID string) error {
	if entity == "" {
		return nil
	}
	policy, ok := s.DocumentVersionPolicies[entity]
	if !ok {
		return Forbidden
	}
	return policy(ctx, q, a, entityID)
}

func (s *Service) authorizeDocumentVersionForID(ctx context.Context, q Querier, a Actor, id string) error {
	var entity, entityID sql.NullString
	if e := q.QueryRowContext(ctx, `SELECT entity_type,entity_id FROM documents WHERE id=?`, id).Scan(&entity, &entityID); e != nil {
		if e == sql.ErrNoRows {
			return Missing
		}
		return e
	}
	if !entity.Valid {
		return nil
	}
	return s.authorizeDocumentVersion(ctx, q, a, entity.String, entityID.String)
}

// documentWriteBinding follows actor -> entity -> document lock ordering.
// Recheck the binding after the document lock so a concurrent bind cannot
// select a different owner policy after authorization.
type documentWriteState struct {
	Owner                  string
	Team, Entity, EntityID sql.NullString
	Version                uint64
	Generated              bool
	Deleted                sql.NullTime
}

func (s *Service) lockDocumentWrite(ctx context.Context, tx *sql.Tx, a Actor, id string, sensitive bool, expected ...string) (documentWriteState, error) {
	var first, current documentWriteState
	if ID(id) != nil {
		return current, Invalid
	}
	const cols = "owner_id,team_id,entity_type,entity_id,version,deleted_at,is_generated"
	scan := func(v *documentWriteState) []any {
		return []any{&v.Owner, &v.Team, &v.Entity, &v.EntityID, &v.Version, &v.Deleted, &v.Generated}
	}
	if e := tx.QueryRowContext(ctx, "SELECT "+cols+" FROM documents WHERE id=?", id).Scan(scan(&first)...); e == sql.ErrNoRows {
		return current, Missing
	} else if e != nil {
		return current, e
	}
	if e := s.Authorize(ctx, tx, a, "core.documents.write", first.Owner, first.Team.String, sensitive); e != nil {
		if e == Forbidden {
			e = Missing
		}
		return current, e
	}
	if len(expected) == 2 && first.Entity.Valid && (first.Entity.String != expected[0] || first.EntityID.String != expected[1]) {
		return current, Conflict
	}
	if first.Entity.Valid {
		policy, ok := s.DocumentPolicies[first.Entity.String]
		if !ok {
			return current, Forbidden
		}
		if e := policy(ctx, tx, a, first.EntityID.String, true); e != nil {
			return current, Missing
		}
	}
	if e := tx.QueryRowContext(ctx, "SELECT "+cols+" FROM documents WHERE id=? FOR UPDATE", id).Scan(scan(&current)...); e == sql.ErrNoRows {
		return current, Missing
	} else if e != nil {
		return current, e
	}
	if current.Owner != first.Owner || current.Team != first.Team || current.Entity != first.Entity || current.EntityID != first.EntityID {
		return current, Conflict
	}
	return current, nil
}

// DocumentBindingAccess reauthorizes current document/destination access on
// every replay. An optional expected owner rejects foreign bindings before
// taking an unrelated entity lock.
func (s *Service) DocumentBindingAccess(ctx context.Context, tx *sql.Tx, a Actor, id string, sensitive bool, expected ...string) (DocumentBinding, error) {
	v, e := s.lockDocumentWrite(ctx, tx, a, id, sensitive, expected...)
	if e != nil {
		return DocumentBinding{}, e
	}
	if v.Deleted.Valid {
		return DocumentBinding{}, Missing
	}
	return DocumentBinding{OwnerID: v.Owner, TeamID: v.Team.String, Version: v.Version, EntityType: v.Entity.String, EntityID: v.EntityID.String, Generated: v.Generated}, nil
}

// BindDocument is the only public cross-module write to document ownership links.
func (s *Service) BindDocument(ctx context.Context, tx *sql.Tx, a Actor, id, entity, entityID string, version uint64) error {
	policy, ok := s.DocumentPolicies[entity]
	if !ok {
		return Forbidden
	}
	if ID(id) != nil || ID(entityID) != nil || version == 0 {
		return Invalid
	}
	if e := policy(ctx, tx, a, entityID, true); e != nil {
		return e
	}
	if bindingPolicy, ok := s.DocumentBindingPolicies[entity]; ok {
		if e := bindingPolicy(ctx, tx, a, entityID); e != nil {
			return e
		}
	}
	var owner string
	var team, kind sql.NullString
	var old uint64
	var deleted sql.NullTime
	var generated bool
	if e := tx.QueryRowContext(ctx, `SELECT owner_id,team_id,entity_type,version,deleted_at,is_generated FROM documents WHERE id=? FOR UPDATE`, id).Scan(&owner, &team, &kind, &old, &deleted, &generated); e != nil {
		if e == sql.ErrNoRows {
			return Missing
		}
		return e
	}
	if e := s.Authorize(ctx, tx, a, "core.documents.write", owner, team.String, true); e != nil {
		return Missing
	}
	if kind.Valid || deleted.Valid || generated || old != version {
		return Conflict
	}
	if e := Changed(tx.ExecContext(ctx, `UPDATE documents SET entity_type=?,entity_id=?,version=version+1 WHERE id=? AND version=?`, entity, entityID, id, version)); e != nil {
		return e
	}
	return s.Audit(ctx, tx, a, "documents.link", "document", id, "success", map[string]string{"entity_type": entity, "entity_id": entityID})
}
