package collaboration

import (
	"context"
	"database/sql"
	"strconv"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func permissionSQL(user, permission, owner, team string) (string, []any) {
	return `EXISTS(SELECT 1 FROM user_roles urx JOIN role_permissions rpx ON rpx.role_id=urx.role_id
		WHERE urx.user_id=? AND rpx.permission=? AND (rpx.scope='all' OR (rpx.scope='self' AND ` + owner + `=?) OR
		(rpx.scope='team' AND ` + team + ` IS NOT NULL AND EXISTS(SELECT 1 FROM team_members tmx WHERE tmx.team_id=` + team + ` AND tmx.user_id=?))))`,
		[]any{user, permission, user, user}
}

func folderOwner(alias string) string {
	return `CASE WHEN ` + alias + `.scope='person' THEN CAST(` + alias + `.person_id AS CHAR) ELSE CAST(` + alias + `.created_by AS CHAR) END`
}

func (s *Service) CreateFolder(ctx context.Context, a core.Actor, key string, p FolderInput) (any, error) {
	p.Name = strings.TrimSpace(p.Name)
	if a.Type != "user" || key == "" || len(key) > 100 || !validName(p.Name, 120) || (p.Scope != "company" && p.Scope != "person") {
		return nil, core.Invalid
	}
	if p.Scope == "person" {
		if core.ID(p.PersonID) != nil || p.PersonID != a.UserID || p.TeamID != "" {
			return nil, core.Forbidden
		}
	} else if p.PersonID != "" {
		return nil, core.Invalid
	}
	if p.TeamID != "" && core.ID(p.TeamID) != nil || p.ParentID != "" && core.ID(p.ParentID) != nil {
		return nil, core.Invalid
	}
	owner := a.UserID
	if p.Scope == "person" {
		owner = p.PersonID
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "colaboracion.organize", owner, p.TeamID, false); e != nil {
			return e
		}
		if p.TeamID != "" {
			var member int
			if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?`, p.TeamID, a.UserID).Scan(&member); e != nil {
				return e
			} else if member == 0 {
				return core.Forbidden
			}
		}
		if p.ParentID != "" {
			parentID := p.ParentID
			for depth := 0; parentID != ""; depth++ {
				if depth >= 8 {
					return core.Invalid
				}
				parent, e := s.folder(ctx, tx, a, parentID, "colaboracion.organize", false)
				if e != nil {
					return e
				}
				if parent.Scope != p.Scope || parent.PersonID != p.PersonID || parent.TeamID != p.TeamID {
					return core.Conflict
				}
				parentID = parent.ParentID
			}
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.folder.create", key, p, func() (any, error) {
			r, e := tx.ExecContext(ctx, `INSERT INTO collaboration_folders(scope,person_id,team_id,parent_id,name,created_by,created_at,updated_at)
				VALUES(?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, p.Scope, nullableString(p.PersonID), nullableString(p.TeamID), nullableString(p.ParentID), p.Name, a.UserID)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			id := core.WrapID(n)
			if e = s.Core.Audit(ctx, tx, a, "collaboration.folder_created", "collaboration_folders", id, "success", map[string]string{"scope": p.Scope}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "collaboration.folder_created", "collaboration_folders", id, 1); e != nil {
				return nil, e
			}
			return s.folder(ctx, tx, a, id, "colaboracion.view", false)
		})
		return e
	})
	return out, e
}

func (s *Service) ListFolders(ctx context.Context, a core.Actor, scope, personID, parentID string, deleted bool) ([]Folder, error) {
	if scope != "" && scope != "company" && scope != "person" || personID != "" && core.ID(personID) != nil || parentID != "" && parentID != "root" && core.ID(parentID) != nil {
		return nil, core.Invalid
	}
	if scope == "company" && personID != "" || scope == "person" && personID == "" {
		return nil, core.Invalid
	}
	if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.view"); e != nil {
		return nil, e
	}
	if deleted {
		if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.delete"); e != nil {
			return nil, e
		}
	}
	where := []string{"1=1"}
	args := []any{}
	if deleted {
		where = append(where, "f.deleted_at IS NOT NULL")
	} else {
		where = append(where, "f.deleted_at IS NULL")
	}
	if scope != "" {
		where = append(where, "f.scope=?")
		args = append(args, scope)
	}
	if personID != "" {
		where = append(where, "f.person_id=?")
		args = append(args, personID)
	}
	if parentID == "root" || parentID == "" {
		where = append(where, "f.parent_id IS NULL")
	} else if parentID != "" {
		parent, e := s.folder(ctx, s.Core.DB, a, parentID, "colaboracion.view", false)
		if e != nil {
			return nil, e
		}
		where = append(where, "f.parent_id=?")
		args = append(args, parent.ID)
	}
	grant, ga := permissionSQL(a.UserID, "colaboracion.view", folderOwner("f"), "CAST(f.team_id AS CHAR)")
	where = append(where, grant)
	args = append(args, ga...)
	if deleted {
		grant, ga = permissionSQL(a.UserID, "colaboracion.delete", folderOwner("f"), "CAST(f.team_id AS CHAR)")
		where = append(where, grant)
		args = append(args, ga...)
	}
	entryView, eva := permissionSQL(a.UserID, "colaboracion.view", `CASE WHEN ce.scope='person' THEN CAST(ce.person_id AS CHAR) ELSE CAST(ce.uploaded_by AS CHAR) END`, "CAST(ce.team_id AS CHAR)")
	documentView, dva := permissionSQL(a.UserID, "core.documents.read", "CAST(ce.uploaded_by AS CHAR)", "CAST(ce.team_id AS CHAR)")
	countArgs := append(append([]any{}, eva...), dva...)
	query := `SELECT CAST(f.id AS CHAR),f.scope,CAST(f.person_id AS CHAR),CAST(f.team_id AS CHAR),CAST(f.parent_id AS CHAR),f.name,CAST(f.created_by AS CHAR),f.version,f.created_at,f.updated_at,f.deleted_at,
		(SELECT COUNT(*) FROM collaboration_entries ce JOIN documents d ON d.id=ce.document_id AND d.entity_type='collaboration' AND d.entity_id=CAST(ce.id AS CHAR)
		 WHERE ce.folder_id=f.id AND d.deleted_at IS NULL AND ` + entryView + ` AND ` + documentView + `)
		FROM collaboration_folders f WHERE ` + strings.Join(where, " AND ") + ` ORDER BY f.name,f.id`
	rows, e := s.Core.DB.QueryContext(ctx, query, append(countArgs, args...)...)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Folder{}
	for rows.Next() {
		var f Folder
		var person, team, parent sql.NullString
		var deletedAt sql.NullTime
		if e = rows.Scan(&f.ID, &f.Scope, &person, &team, &parent, &f.Name, &f.CreatedBy, &f.Version, &f.CreatedAt, &f.UpdatedAt, &deletedAt, &f.FileCount); e != nil {
			return nil, e
		}
		f.PersonID, f.TeamID, f.ParentID = person.String, team.String, parent.String
		if deletedAt.Valid {
			f.DeletedAt = &deletedAt.Time
		}
		out = append(out, f)
	}
	return out, rows.Err()
}

func (s *Service) UpdateFolder(ctx context.Context, a core.Actor, id, key string, p FolderUpdate) (any, error) {
	p.Name = strings.TrimSpace(p.Name)
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) || !validName(p.Name, 120) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		f, e := s.folder(ctx, tx, a, id, "colaboracion.organize", false)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.folder.update:"+id, key, p, func() (any, error) {
			var current uint64
			if e := tx.QueryRowContext(ctx, `SELECT version FROM collaboration_folders WHERE id=? FOR UPDATE`, id).Scan(&current); e != nil {
				return nil, e
			}
			if current != p.Version || f.Version != p.Version {
				return nil, core.Conflict
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_folders SET name=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND deleted_at IS NULL`, p.Name, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.folder_renamed", "collaboration_folders", id, "success", map[string]string{"version": coreVersion(p.Version + 1)}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.folder_renamed", "collaboration_folders", id, p.Version+1); e != nil {
				return nil, e
			}
			return s.folder(ctx, tx, a, id, "colaboracion.view", false)
		})
		return e
	})
	return out, e
}

func coreVersion(v uint64) string { return strconv.FormatUint(v, 10) }

func (s *Service) TrashFolder(ctx context.Context, a core.Actor, id, key string, p VersionInput, restore bool) (any, error) {
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		f, e := s.folder(ctx, tx, a, id, "colaboracion.delete", true)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.folder.trash:"+id, key, map[string]any{"version": p.Version, "restore": restore}, func() (any, error) {
			if restore != (f.DeletedAt != nil) {
				return nil, core.Conflict
			}
			var version uint64
			var deleted sql.NullTime
			if e := tx.QueryRowContext(ctx, `SELECT version,deleted_at FROM collaboration_folders WHERE id=? FOR UPDATE`, id).Scan(&version, &deleted); e != nil {
				return nil, e
			}
			if version != p.Version || restore != deleted.Valid {
				return nil, core.Conflict
			}
			if !restore {
				var count int
				if e := tx.QueryRowContext(ctx, `SELECT (SELECT COUNT(*) FROM collaboration_entries WHERE folder_id=?)+(SELECT COUNT(*) FROM collaboration_folders WHERE parent_id=? AND deleted_at IS NULL)`, id, id).Scan(&count); e != nil {
					return nil, e
				}
				if count != 0 {
					return nil, core.Conflict
				}
			} else if f.ParentID != "" {
				if _, e := s.folder(ctx, tx, a, f.ParentID, "colaboracion.view", false); e != nil {
					return nil, core.Conflict
				}
			}
			var deletedAt any
			if !restore {
				deletedAt = time.Now().UTC()
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_folders SET deleted_at=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, deletedAt, id, p.Version)); e != nil {
				return nil, e
			}
			action := "collaboration.folder_deleted"
			if restore {
				action = "collaboration.folder_restored"
			}
			if e := s.Core.Audit(ctx, tx, a, action, "collaboration_folders", id, "success", map[string]bool{"restored": restore}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, action, "collaboration_folders", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": coreVersion(p.Version + 1)}, nil
		})
		return e
	})
	return out, e
}
