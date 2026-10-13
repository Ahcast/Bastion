package collaboration

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type OwnerResourceSource interface {
	CollaborationResource(context.Context, core.Querier, string) (owner, team, label string, version uint64, err error)
}

// OwnerResourcePolicy adapts a business-owned read port without exposing that
// owner's storage to Collaboration. The owner module remains authoritative.
func OwnerResourcePolicy(permission string, source OwnerResourceSource) ResourcePolicy {
	return func(ctx context.Context, q core.Querier, id string) (Resource, error) {
		if source == nil || core.ID(id) != nil {
			return Resource{}, core.Missing
		}
		owner, team, label, version, e := source.CollaborationResource(ctx, q, id)
		if e != nil {
			return Resource{}, e
		}
		return Resource{ID: id, OwnerID: owner, TeamID: team, ViewPermission: permission, Label: label, Version: version}, nil
	}
}

// CollaborationEntryResource validates a live library record and its Core.Files
// binding. Authorization is applied by resource() for the current caller.
func (s *Service) CollaborationEntryResource(ctx context.Context, q core.Querier, id string) (Resource, error) {
	var out Resource
	if core.ID(id) != nil {
		return out, core.Missing
	}
	var scope, uploadedBy, documentOwner, title string
	var documentID string
	var version uint64
	var person, team, entryFolder, folderID, folderScope, folderPerson, folderTeam sql.NullString
	var folderDeleted sql.NullTime
	var documentTeam, documentEntity, documentEntityID sql.NullString
	var generated bool
	e := q.QueryRowContext(ctx, `SELECT CAST(e.id AS CHAR),e.scope,CAST(e.uploaded_by AS CHAR),CAST(e.person_id AS CHAR),CAST(e.team_id AS CHAR),CAST(e.folder_id AS CHAR),CAST(e.document_id AS CHAR),e.title,e.version,
		CAST(d.owner_id AS CHAR),CAST(d.team_id AS CHAR),d.entity_type,d.entity_id,d.is_generated,
		CAST(f.id AS CHAR),f.scope,CAST(f.person_id AS CHAR),CAST(f.team_id AS CHAR),f.deleted_at
		FROM collaboration_entries e JOIN documents d ON d.id=e.document_id LEFT JOIN collaboration_folders f ON f.id=e.folder_id
		WHERE e.id=? AND d.deleted_at IS NULL`, id).Scan(&out.ID, &scope, &uploadedBy, &person, &team, &entryFolder,
		&documentID, &title, &version, &documentOwner, &documentTeam, &documentEntity, &documentEntityID, &generated,
		&folderID, &folderScope, &folderPerson, &folderTeam, &folderDeleted)
	if e == sql.ErrNoRows {
		return Resource{}, core.Missing
	}
	if e != nil {
		return Resource{}, e
	}
	if generated || !documentEntity.Valid || documentEntity.String != "collaboration" || documentEntityID.String != id || documentOwner != uploadedBy || documentTeam.String != team.String {
		return Resource{}, core.Missing
	}
	if scope == "person" {
		if !person.Valid || person.String != uploadedBy || team.Valid || documentTeam.Valid {
			return Resource{}, core.Missing
		}
		out.OwnerID = person.String
	} else if scope == "company" {
		if person.Valid {
			return Resource{}, core.Missing
		}
		out.OwnerID = uploadedBy
	} else {
		return Resource{}, core.Missing
	}
	if entryFolder.Valid {
		if !folderID.Valid || folderID.String != entryFolder.String || !folderScope.Valid || folderDeleted.Valid ||
			folderScope.String != scope || folderPerson.String != person.String || folderTeam.String != team.String {
			return Resource{}, core.Missing
		}
	} else if folderID.Valid {
		return Resource{}, core.Missing
	}
	out.TeamID = team.String
	out.Label, out.Version = title, version
	out.DocumentID, out.DocumentOwnerID, out.DocumentTeamID = documentID, documentOwner, documentTeam.String
	return out, nil
}

func (s *Service) resource(ctx context.Context, q core.Querier, a core.Actor, resourceType, id string) (Resource, error) {
	if a.Type != "user" || core.ID(id) != nil {
		return Resource{}, core.Missing
	}
	policy, ok := s.ResourcePolicies[resourceType]
	if !ok {
		return Resource{}, core.Missing
	}
	r, e := policy(ctx, q, id)
	if e != nil {
		return Resource{}, e
	}
	if r.ID != id {
		return Resource{}, core.Missing
	}
	if e = s.Core.Authorize(ctx, q, a, "colaboracion.view", r.OwnerID, r.TeamID, false); e == core.Forbidden {
		return Resource{}, core.Missing
	} else if e != nil {
		return Resource{}, e
	}
	if r.ViewPermission != "" {
		if e = s.Core.Authorize(ctx, q, a, r.ViewPermission, r.OwnerID, r.TeamID, false); e == core.Forbidden {
			return Resource{}, core.Missing
		} else if e != nil {
			return Resource{}, e
		}
	}
	if r.DocumentID != "" {
		if e = s.Core.Authorize(ctx, q, a, "core.documents.read", r.DocumentOwnerID, r.DocumentTeamID, false); e == core.Forbidden {
			return Resource{}, core.Missing
		} else if e != nil {
			return Resource{}, e
		}
	}
	return r, nil
}

func resourcePermission(ctx context.Context, q core.Querier, s *Service, a core.Actor, r Resource, permission string, sensitive bool) error {
	return s.Core.Authorize(ctx, q, a, permission, r.OwnerID, r.TeamID, sensitive)
}

func (s *Service) grantHolderCanResource(ctx context.Context, q core.Querier, userID string, r Resource) error {
	if e := s.Core.AuthorizeGrantHolder(ctx, q, userID, "colaboracion.view", r.OwnerID, r.TeamID); e != nil {
		return e
	}
	if r.ViewPermission != "" {
		if e := s.Core.AuthorizeGrantHolder(ctx, q, userID, r.ViewPermission, r.OwnerID, r.TeamID); e != nil {
			return e
		}
	}
	if r.DocumentID != "" {
		if e := s.Core.AuthorizeGrantHolder(ctx, q, userID, "core.documents.read", r.DocumentOwnerID, r.DocumentTeamID); e != nil {
			return e
		}
	}
	return nil
}

func (s *Service) MentionableUsers(ctx context.Context, a core.Actor, resourceType, id, search string) ([]MentionableUser, error) {
	if len([]rune(search)) > 100 || strings.ContainsRune(search, '\x00') {
		return nil, core.Invalid
	}
	r, e := s.resource(ctx, s.Core.DB, a, resourceType, id)
	if e != nil {
		return nil, e
	}
	if e = resourcePermission(ctx, s.Core.DB, s, a, r, "colaboracion.comment", false); e != nil {
		return nil, e
	}
	where := []string{"active=TRUE", "totp_secret IS NOT NULL", "CAST(id AS CHAR)<>?"}
	args := []any{a.UserID}
	if search != "" {
		pattern := "%" + strings.NewReplacer("!", "!!", "%", "!%", "_", "!_").Replace(search) + "%"
		where = append(where, "name LIKE ? ESCAPE '!'")
		args = append(args, pattern)
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(id AS CHAR),name FROM users WHERE `+strings.Join(where, " AND ")+` ORDER BY name,id LIMIT 200`, args...)
	if e != nil {
		return nil, e
	}
	candidates := []MentionableUser{}
	for rows.Next() {
		var item MentionableUser
		if e = rows.Scan(&item.ID, &item.Name); e != nil {
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
	out := []MentionableUser{}
	for _, item := range candidates {
		if e = s.grantHolderCanResource(ctx, s.Core.DB, item.ID, r); e != nil {
			if e == core.Forbidden {
				continue
			}
			return nil, e
		}
		out = append(out, item)
		if len(out) == 50 {
			break
		}
	}
	return out, nil
}
