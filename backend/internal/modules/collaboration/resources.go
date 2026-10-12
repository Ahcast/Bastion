package collaboration

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

// CollaborationEntryResource is the U2 resource policy registered by
// composition. The lookup validates the live Core.Files binding but does not
// authorize the caller; resource() applies the current caller's grants after
// loading the owning module's security context.
func (s *Service) CollaborationEntryResource(ctx context.Context, q core.Querier, id string) (Resource, error) {
	var out Resource
	if core.ID(id) != nil {
		return out, core.Missing
	}
	var scope, uploadedBy, documentOwner string
	var documentID string
	var person, team, entryFolder, folderID, folderScope, folderPerson, folderTeam sql.NullString
	var folderDeleted sql.NullTime
	var documentTeam, documentEntity, documentEntityID sql.NullString
	var generated bool
	e := q.QueryRowContext(ctx, `SELECT CAST(e.id AS CHAR),e.scope,CAST(e.uploaded_by AS CHAR),CAST(e.person_id AS CHAR),CAST(e.team_id AS CHAR),CAST(e.folder_id AS CHAR),CAST(e.document_id AS CHAR),
		CAST(d.owner_id AS CHAR),CAST(d.team_id AS CHAR),d.entity_type,d.entity_id,d.is_generated,
		CAST(f.id AS CHAR),f.scope,CAST(f.person_id AS CHAR),CAST(f.team_id AS CHAR),f.deleted_at
		FROM collaboration_entries e JOIN documents d ON d.id=e.document_id LEFT JOIN collaboration_folders f ON f.id=e.folder_id
		WHERE e.id=? AND d.deleted_at IS NULL`, id).Scan(&out.ID, &scope, &uploadedBy, &person, &team, &entryFolder,
		&documentID, &documentOwner, &documentTeam, &documentEntity, &documentEntityID, &generated,
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
	if e = s.Core.Authorize(ctx, q, a, "core.documents.read", r.DocumentOwnerID, r.DocumentTeamID, false); e == core.Forbidden {
		return Resource{}, core.Missing
	} else if e != nil {
		return Resource{}, e
	}
	return r, nil
}

func resourcePermission(ctx context.Context, q core.Querier, s *Service, a core.Actor, r Resource, permission string, sensitive bool) error {
	if e := s.Core.Authorize(ctx, q, a, permission, r.OwnerID, r.TeamID, sensitive); e != nil {
		return e
	}
	return nil
}

func mentionGrantClause(permission, owner, team string) (string, []any) {
	return `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id
		WHERE ur.user_id=u.id AND rp.permission=? AND (rp.scope='all' OR (rp.scope='self' AND CAST(u.id AS CHAR)=?) OR
		(rp.scope='team' AND ?<>'' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=? AND tm.user_id=u.id))))`,
		[]any{permission, owner, team, team}
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
	collab, collabArgs := mentionGrantClause("colaboracion.view", r.OwnerID, r.TeamID)
	docs, docArgs := mentionGrantClause("core.documents.read", r.DocumentOwnerID, r.DocumentTeamID)
	where := []string{"u.active=TRUE", "u.totp_secret IS NOT NULL", "CAST(u.id AS CHAR)<>?", collab, docs}
	args := []any{a.UserID}
	args = append(args, collabArgs...)
	args = append(args, docArgs...)
	if search != "" {
		pattern := "%" + strings.NewReplacer("!", "!!", "%", "!%", "_", "!_").Replace(search) + "%"
		where = append(where, "u.name LIKE ? ESCAPE '!'")
		args = append(args, pattern)
	}
	// EXISTS predicates avoid grant-row duplication, so DISTINCT is unnecessary
	// and would conflict with MySQL 5.7's ONLY_FULL_GROUP_BY ORDER BY rules.
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(u.id AS CHAR),u.name FROM users u WHERE `+strings.Join(where, " AND ")+` ORDER BY u.name,u.id LIMIT 50`, args...)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []MentionableUser{}
	for rows.Next() {
		var item MentionableUser
		if e = rows.Scan(&item.ID, &item.Name); e != nil {
			return nil, e
		}
		out = append(out, item)
	}
	return out, rows.Err()
}
