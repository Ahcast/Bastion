package core

import (
	"context"
	"database/sql"
)

// FileEvidence freezes metadata, never object keys, signed URLs or bytes.
type FileEvidence struct {
	ID       string `json:"id"`
	Version  uint64 `json:"version,string"`
	Number   int    `json:"file_version"`
	Name     string `json:"filename"`
	Media    string `json:"media_type"`
	Size     int64  `json:"size_bytes"`
	Checksum string `json:"checksum"`
	Status   string `json:"status"`
}

// EntityFileEvidence is a transactional internal contract for business approval.
// It includes every active uploaded attachment, not a permission-filtered subset.
func (s *Service) EntityFileEvidence(ctx context.Context, tx *sql.Tx, a Actor, entity, id string) ([]FileEvidence, error) {
	out := []FileEvidence{}
	rows, e := tx.QueryContext(ctx, `SELECT d.id,d.version,d.current_version,v.filename,v.media_type,v.size_bytes,v.checksum,v.status FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version WHERE d.entity_type=? AND d.entity_id=? AND d.deleted_at IS NULL AND d.is_generated=FALSE ORDER BY d.id FOR UPDATE`, entity, id)
	if e != nil {
		return nil, e
	}
	for rows.Next() {
		var f FileEvidence
		if e = rows.Scan(&f.ID, &f.Version, &f.Number, &f.Name, &f.Media, &f.Size, &f.Checksum, &f.Status); e != nil {
			rows.Close()
			return nil, e
		}
		out = append(out, f)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return nil, e
	}
	for _, f := range out {
		if _, _, e = s.DocumentAccess(ctx, tx, a, f.ID, "core.documents.read"); e != nil {
			return nil, e
		}
		if f.Status != "available" {
			return nil, Conflict
		}
	}
	return out, nil
}
