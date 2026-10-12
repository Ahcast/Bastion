package collaboration

import (
	"context"
	"database/sql"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func (s *Service) CreateEntry(ctx context.Context, a core.Actor, key string, p EntryInput) (any, error) {
	p.Title, p.Notes = strings.TrimSpace(p.Title), strings.TrimSpace(p.Notes)
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(p.DocumentID) != nil || p.DocumentVersion == 0 || p.DocumentVersion == ^uint64(0) ||
		(p.Scope != "company" && p.Scope != "person") || !validName(p.Title, 180) || len([]rune(p.Notes)) > 4000 || strings.ContainsRune(p.Notes, '\x00') ||
		(p.PersonID != "" && core.ID(p.PersonID) != nil) || (p.FolderID != "" && core.ID(p.FolderID) != nil) {
		return nil, core.Invalid
	}
	if p.Scope == "person" {
		if p.PersonID != a.UserID {
			return nil, core.Forbidden
		}
	} else if p.PersonID != "" {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		binding, e := s.Core.DocumentBindingAccess(ctx, tx, a, p.DocumentID, true)
		if e != nil {
			return e
		}
		replay := binding.EntityType == "collaboration"
		var folder Folder
		if replay {
			// BindDocument increments the Core document version. Reauthorize the
			// already-bound entry before consulting idempotency so an exact lost-ACK
			// replay succeeds without weakening current access checks.
			entry, e := s.entry(ctx, tx, a, binding.EntityID, "colaboracion.organize")
			if e != nil {
				return e
			}
			if entry.DocumentID != p.DocumentID || entry.Scope != p.Scope || entry.PersonID != p.PersonID || entry.FolderID != p.FolderID || entry.Title != p.Title || entry.Notes != p.Notes {
				return core.Conflict
			}
			if binding.OwnerID != entry.UploadedBy || binding.TeamID != entry.TeamID {
				return core.Missing
			}
			if e = s.gate(ctx, tx, a, "colaboracion.upload", entry.UploadedBy, entry.TeamID, true); e != nil {
				return e
			}
			if e = s.gate(ctx, tx, a, "colaboracion.organize", entry.UploadedBy, entry.TeamID, true); e != nil {
				return e
			}
		} else {
			if binding.OwnerID != a.UserID || binding.Generated || binding.Version != p.DocumentVersion || binding.EntityType != "" {
				return core.Conflict
			}
			var fileStatus string
			if e = tx.QueryRowContext(ctx, `SELECT status FROM document_versions WHERE document_id=? AND number=(SELECT current_version FROM documents WHERE id=?)`, p.DocumentID, p.DocumentID).Scan(&fileStatus); e != nil {
				return e
			}
			if fileStatus != "quarantined" && fileStatus != "available" {
				return core.Conflict
			}
			if p.Scope == "person" && (binding.OwnerID != p.PersonID || binding.TeamID != "") {
				return core.Forbidden
			}
			if e = s.gate(ctx, tx, a, "colaboracion.upload", binding.OwnerID, binding.TeamID, true); e != nil {
				return e
			}
			if e = s.gate(ctx, tx, a, "colaboracion.organize", binding.OwnerID, binding.TeamID, true); e != nil {
				return e
			}
			if p.FolderID != "" {
				folder, e = s.folder(ctx, tx, a, p.FolderID, "colaboracion.organize", false)
				if e != nil {
					return e
				}
				if folder.Scope != p.Scope || folder.PersonID != p.PersonID || folder.TeamID != binding.TeamID {
					return core.Conflict
				}
			}
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.entry.create", key, p, func() (any, error) {
			if replay {
				// An exact replay returns above. A second key cannot create a second
				// library row for an already-bound document.
				return nil, core.Conflict
			}
			r, e := tx.ExecContext(ctx, `INSERT INTO collaboration_entries(document_id,folder_id,scope,person_id,team_id,title,notes,uploaded_by,created_at,updated_at)
				VALUES(?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, p.DocumentID, nullableString(p.FolderID), p.Scope, nullableString(p.PersonID), nullableString(binding.TeamID), p.Title, nullableString(p.Notes), a.UserID)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			entryID := core.WrapID(n)
			if e = s.Core.BindDocument(ctx, tx, a, p.DocumentID, "collaboration", entryID, p.DocumentVersion); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "collaboration.file_linked", "collaboration_entries", entryID, "success", map[string]string{"document_id": p.DocumentID, "scope": p.Scope}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "collaboration.file_linked", "collaboration_entries", entryID, 1); e != nil {
				return nil, e
			}
			return s.entryWithMetadata(ctx, tx, a, entryID, false)
		})
		return e
	})
	return out, e
}

func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (Page, error) {
	out := Page{Data: []Entry{}, Page: f.Page, PerPage: f.PerPage}
	if !pageBounds(f.Page, f.PerPage) || (f.Scope != "" && f.Scope != "company" && f.Scope != "person") ||
		(f.PersonID != "" && core.ID(f.PersonID) != nil) || (f.FolderSet && f.FolderID != "" && f.FolderID != "root" && core.ID(f.FolderID) != nil) ||
		len([]rune(f.Search)) > 100 || strings.ContainsRune(f.Search, '\x00') ||
		(f.Scope == "company" && f.PersonID != "") || (f.Scope == "person" && f.PersonID == "") {
		return out, core.Invalid
	}
	if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.view"); e != nil {
		return out, e
	}
	if f.Deleted {
		if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.delete"); e != nil {
			return out, e
		}
	}
	where := []string{
		"d.entity_type='collaboration'",
		"d.entity_id=CAST(e.id AS CHAR)",
		"d.is_generated=FALSE",
		"((e.folder_id IS NULL) OR (f.id IS NOT NULL AND f.deleted_at IS NULL))",
	}
	args := []any{}
	if f.Deleted {
		where = append(where, "d.deleted_at IS NOT NULL")
	} else {
		where = append(where, "d.deleted_at IS NULL")
	}
	if f.Scope != "" {
		where = append(where, "e.scope=?")
		args = append(args, f.Scope)
	}
	if f.PersonID != "" {
		where = append(where, "e.person_id=?")
		args = append(args, f.PersonID)
	}
	if f.FolderSet {
		if f.FolderID == "" || f.FolderID == "root" {
			where = append(where, "e.folder_id IS NULL")
		} else {
			folder, e := s.folder(ctx, s.Core.DB, a, f.FolderID, "colaboracion.view", false)
			if e != nil {
				return out, e
			}
			where = append(where, "e.folder_id=?")
			args = append(args, folder.ID)
		}
	}
	if f.Search != "" {
		search := strings.NewReplacer("!", "!!", "%", "!%", "_", "!_").Replace(f.Search)
		where = append(where, "(e.title LIKE ? ESCAPE '!' OR COALESCE(e.notes,'') LIKE ? ESCAPE '!' OR v.filename LIKE ? ESCAPE '!')")
		pattern := "%" + search + "%"
		args = append(args, pattern, pattern, pattern)
	}
	collabPermission, collabArgs := permissionSQL(a.UserID, "colaboracion.view", `CASE WHEN e.scope='person' THEN CAST(e.person_id AS CHAR) ELSE CAST(e.uploaded_by AS CHAR) END`, "CAST(e.team_id AS CHAR)")
	docPermission, docArgs := permissionSQL(a.UserID, "core.documents.read", "CAST(d.owner_id AS CHAR)", "CAST(d.team_id AS CHAR)")
	where = append(where, collabPermission, docPermission)
	args = append(args, collabArgs...)
	args = append(args, docArgs...)
	from := ` FROM collaboration_entries e JOIN documents d ON d.id=e.document_id JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version LEFT JOIN collaboration_folders f ON f.id=e.folder_id WHERE ` + strings.Join(where, " AND ")
	if e := s.Core.DB.QueryRowContext(ctx, "SELECT COUNT(*)"+from, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(e.id AS CHAR),CAST(e.document_id AS CHAR),CAST(e.folder_id AS CHAR),e.scope,CAST(e.person_id AS CHAR),CAST(e.team_id AS CHAR),e.title,COALESCE(e.notes,''),CAST(e.uploaded_by AS CHAR),e.version,e.updated_at,d.version,d.current_version,v.filename,v.media_type,v.size_bytes,v.status,d.deleted_at IS NOT NULL`+from+` ORDER BY e.updated_at DESC,e.id DESC LIMIT ? OFFSET ?`, append(args, f.PerPage, (f.Page-1)*f.PerPage)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var item Entry
		var folder, person, team sql.NullString
		var deleted bool
		if e = rows.Scan(&item.ID, &item.DocumentID, &folder, &item.Scope, &person, &team, &item.Title, &item.Notes, &item.UploadedBy, &item.Version, &item.UpdatedAt, &item.DocumentVersion, &item.FileVersion, &item.Filename, &item.MediaType, &item.SizeBytes, &item.Status, &deleted); e != nil {
			return out, e
		}
		item.FolderID, item.PersonID, item.TeamID, item.Deleted = folder.String, person.String, team.String, deleted
		out.Data = append(out.Data, item)
	}
	return out, rows.Err()
}

func (s *Service) UpdateEntry(ctx context.Context, a core.Actor, id, key string, p EntryUpdate) (any, error) {
	p.Title, p.Notes = strings.TrimSpace(p.Title), strings.TrimSpace(p.Notes)
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) ||
		!validName(p.Title, 180) || len([]rune(p.Notes)) > 4000 || strings.ContainsRune(p.Notes, '\x00') ||
		(p.FolderID != "" && core.ID(p.FolderID) != nil) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		r, e := s.entry(ctx, tx, a, id, "colaboracion.organize")
		if e != nil {
			return e
		}
		var lockedID string
		if e = tx.QueryRowContext(ctx, `SELECT CAST(id AS CHAR) FROM collaboration_entries WHERE id=? FOR UPDATE`, id).Scan(&lockedID); e == sql.ErrNoRows {
			return core.Missing
		} else if e != nil {
			return e
		}
		binding, e := s.Core.DocumentBindingAccess(ctx, tx, a, r.DocumentID, true, "collaboration", id)
		if e != nil {
			return e
		}
		if binding.OwnerID != r.UploadedBy || binding.TeamID != r.TeamID {
			return core.Missing
		}
		if e = s.gate(ctx, tx, a, "colaboracion.organize", r.UploadedBy, r.TeamID, true); e != nil {
			return e
		}
		var folder Folder
		if p.FolderID != "" {
			folder, e = s.folder(ctx, tx, a, p.FolderID, "colaboracion.organize", false)
			if e != nil {
				return e
			}
			if folder.Scope != r.Scope || folder.PersonID != r.PersonID || folder.TeamID != r.TeamID {
				return core.Conflict
			}
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.entry.update:"+id, key, p, func() (any, error) {
			var current uint64
			if e := tx.QueryRowContext(ctx, `SELECT version FROM collaboration_entries WHERE id=? FOR UPDATE`, id).Scan(&current); e != nil {
				if e == sql.ErrNoRows {
					return nil, core.Missing
				}
				return nil, e
			}
			if current != p.Version || r.Version != p.Version {
				return nil, core.Conflict
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_entries SET folder_id=?,title=?,notes=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, nullableString(p.FolderID), p.Title, nullableString(p.Notes), id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.file_updated", "collaboration_entries", id, "success", map[string]string{"version": coreVersion(p.Version + 1)}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.file_updated", "collaboration_entries", id, p.Version+1); e != nil {
				return nil, e
			}
			return s.entryWithMetadata(ctx, tx, a, id, false)
		})
		return e
	})
	return out, e
}

func (s *Service) TrashEntry(ctx context.Context, a core.Actor, id, key string, p VersionInput, restore bool) (any, error) {
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) {
		return nil, core.Invalid
	}
	r, e := s.entry(ctx, s.Core.DB, a, id, "colaboracion.delete")
	if e != nil {
		return nil, e
	}
	if e = s.gate(ctx, s.Core.DB, a, "colaboracion.delete", r.UploadedBy, r.TeamID, true); e != nil {
		return nil, e
	}
	return s.Core.TrashFile(ctx, a, r.DocumentID, key, p.Version, restore)
}

func (s *Service) Deliver(ctx context.Context, a core.Actor, id string, number int, preview bool, objects core.Objects) (any, error) {
	r, e := s.entry(ctx, s.Core.DB, a, id, "colaboracion.view")
	if e != nil {
		return nil, e
	}
	if number < 0 {
		return nil, core.Invalid
	}
	if preview {
		return s.Core.PreviewFile(ctx, a, r.DocumentID, number, objects)
	}
	return s.Core.DownloadFile(ctx, a, r.DocumentID, number, objects)
}
