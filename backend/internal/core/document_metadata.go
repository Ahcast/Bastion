package core

import (
	"context"
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
