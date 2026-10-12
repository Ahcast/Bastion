package core

import (
	"context"
	"database/sql"
)

type DocumentMetadata struct {
	ID       string `json:"id"`
	Version  uint64 `json:"version,string"`
	Number   int    `json:"file_version"`
	Filename string `json:"filename"`
	Media    string `json:"media_type"`
	Size     int64  `json:"size_bytes"`
	Checksum string `json:"checksum"`
	Status   string `json:"status"`
}

// DocumentMetadata returns metadata only after the document grant and the
// current entity policy have both been authorized. It never returns object keys
// or signed URLs.
func (s *Service) DocumentMetadata(ctx context.Context, q Querier, a Actor, id string) (DocumentMetadata, error) {
	var out DocumentMetadata
	number, version, e := s.DocumentAccess(ctx, q, a, id, "core.documents.read")
	if e != nil {
		return out, e
	}
	out.ID, out.Number, out.Version = id, number, version
	e = q.QueryRowContext(ctx, `SELECT filename,media_type,size_bytes,checksum,status FROM document_versions WHERE document_id=? AND number=?`, id, number).Scan(&out.Filename, &out.Media, &out.Size, &out.Checksum, &out.Status)
	return out, e
}

type DocumentMetadataPage struct {
	Data    []DocumentMetadata `json:"data"`
	Total   int                `json:"total"`
	Page    int                `json:"page"`
	PerPage int                `json:"per_page"`
}

// EntityDocumentMetadataPage applies document scope in SQL before pagination.
// The current domain policy also controls access to deleted-document metadata.
func (s *Service) EntityDocumentMetadataPage(ctx context.Context, q Querier, a Actor, entity, id string, page, limit int, deleted bool) (DocumentMetadataPage, error) {
	out := DocumentMetadataPage{Data: []DocumentMetadata{}, Page: page, PerPage: limit}
	if ID(id) != nil || page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return out, Invalid
	}
	policy, ok := s.DocumentPolicies[entity]
	if !ok {
		return out, Forbidden
	}
	if e := policy(ctx, q, a, id, false); e != nil {
		return out, e
	}
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return out, e
	}
	if e = s.Authorize(ctx, q, a, "core.documents.read", a.UserID, team, false); e != nil {
		return out, e
	}
	w := `d.entity_type=? AND d.entity_id=? AND d.is_generated=FALSE AND EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='core.documents.read' AND (rp.scope='all' OR (rp.scope='self' AND d.owner_id=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=d.team_id AND tm.user_id=?))))`
	if deleted {
		w += ` AND d.deleted_at IS NOT NULL`
	} else {
		w += ` AND d.deleted_at IS NULL`
	}
	args := []any{entity, id, a.UserID, a.UserID, a.UserID}
	from := ` FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version WHERE `
	if e = q.QueryRowContext(ctx, `SELECT COUNT(*)`+from+w, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := q.QueryContext(ctx, `SELECT CAST(d.id AS CHAR),d.version,d.current_version,v.filename,v.media_type,v.size_bytes,v.checksum,v.status`+from+w+` ORDER BY d.id DESC LIMIT ? OFFSET ?`, append(args, limit, (page-1)*limit)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var d DocumentMetadata
		if e = rows.Scan(&d.ID, &d.Version, &d.Number, &d.Filename, &d.Media, &d.Size, &d.Checksum, &d.Status); e != nil {
			return out, e
		}
		out.Data = append(out.Data, d)
	}
	return out, rows.Err()
}
