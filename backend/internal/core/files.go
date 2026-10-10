package core

import (
	"context"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"net/http"
	"path/filepath"
	"strings"
	"time"
)

type ObjectInfo struct {
	Size     int64
	Checksum string
	Prefix   []byte
}
type Objects interface {
	UploadURL(context.Context, string, string, int64, string) (string, error)
	Info(context.Context, string) (ObjectInfo, error)
	DownloadURL(context.Context, string, string) (string, error)
}
type FileRequest struct {
	Filename  string `json:"filename"`
	MediaType string `json:"media_type"`
	Size      int64  `json:"size_bytes"`
	Checksum  string `json:"checksum"`
	Version   uint64 `json:"version,string"`
}

var extensions = map[string]string{"application/pdf": ".pdf", "image/png": ".png", "image/jpeg": ".jpg", "text/plain": ".txt"}

func ValidateFile(c FileRequest) error {
	ext, ok := extensions[c.MediaType]
	if !ok || c.Size < 1 || c.Size > 20*1024*1024 || len(c.Filename) < 1 || len(c.Filename) > 180 || filepath.Base(c.Filename) != c.Filename || strings.ContainsAny(c.Filename, "\\\\\r\n\x00\"") {
		return Invalid
	}
	if strings.ToLower(filepath.Ext(c.Filename)) != ext && !(c.MediaType == "image/jpeg" && strings.ToLower(filepath.Ext(c.Filename)) == ".jpeg") {
		return Invalid
	}
	h, e := hex.DecodeString(c.Checksum)
	if e != nil || len(h) != 32 {
		return Invalid
	}
	return nil
}
func (s *Service) DocumentAccess(ctx context.Context, q Querier, a Actor, id, permission string) (int, uint64, error) {
	var owner string
	var team, entity, eid sql.NullString
	var number int
	var version uint64
	var deleted sql.NullTime
	if e := q.QueryRowContext(ctx, "SELECT owner_id,team_id,entity_type,entity_id,current_version,version,deleted_at FROM documents WHERE id=?", id).Scan(&owner, &team, &entity, &eid, &number, &version, &deleted); e != nil {
		if e == sql.ErrNoRows {
			return 0, 0, Missing
		}
		return 0, 0, e
	}
	if deleted.Valid {
		return 0, 0, Missing
	}
	if e := s.Authorize(ctx, q, a, permission, owner, team.String, false); e != nil {
		return 0, 0, Missing
	}
	if entity.Valid {
		policy, ok := s.DocumentPolicies[entity.String]
		if !ok {
			return 0, 0, Forbidden
		}
		if e := policy(ctx, q, a, eid.String, permission == "core.documents.write"); e != nil {
			return 0, 0, Missing
		}
	}
	return number, version, nil
}
func (s *Service) CreateFile(ctx context.Context, a Actor, id, key string, c FileRequest, objects Objects) (any, error) {
	if !s.IntegrationEnabled(ctx, "files") || objects == nil {
		return nil, Disabled
	}
	if e := ValidateFile(c); e != nil {
		return nil, e
	}
	var result any
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		if id == "" {
			if e := s.Authorize(ctx, tx, a, "core.documents.write", a.UserID, "", false); e != nil {
				return e
			}
		} else {
			if ID(id) != nil {
				return Invalid
			}
			if _, _, e := s.DocumentAccess(ctx, tx, a, id, "core.documents.write"); e != nil {
				return e
			}
		}
		var err error
		result, err = s.Idempotent(ctx, tx, a, "files.create:"+id, key, c, func() (any, error) {
			number := 1
			docID := id
			if docID == "" {
				r, e := tx.ExecContext(ctx, "INSERT INTO documents(owner_id,created_at) VALUES(?,UTC_TIMESTAMP(6))", a.UserID)
				if e != nil {
					return nil, e
				}
				n, _ := r.LastInsertId()
				docID = WrapID(n)
			} else {
				var old int
				var generated bool
				var version uint64
				if e := tx.QueryRowContext(ctx, "SELECT current_version,version,is_generated FROM documents WHERE id=? FOR UPDATE", docID).Scan(&old, &version, &generated); e != nil {
					return nil, e
				}
				if generated || version != c.Version {
					return nil, Conflict
				}
				number = old + 1
				if e := Changed(tx.ExecContext(ctx, "UPDATE documents SET current_version=?,version=version+1 WHERE id=? AND version=?", number, docID, c.Version)); e != nil {
					return nil, e
				}
			}
			objectKey := "documents/" + docID + "/" + fmt.Sprint(number) + "/" + security.ID()
			if _, e := tx.ExecContext(ctx, "INSERT INTO document_versions VALUES(?,?,?,?,?,?,?,'pending',NULL,UTC_TIMESTAMP(6))", docID, number, objectKey, c.Filename, c.MediaType, c.Size, c.Checksum); e != nil {
				return nil, e
			}
			if e := s.Audit(ctx, tx, a, "files.create", "document", docID, "pending", map[string]any{"number": number}); e != nil {
				return nil, e
			}
			return map[string]any{"id": docID, "number": number, "status": "pending", "object_key": objectKey}, nil
		})
		return err
	})
	if e != nil {
		return nil, e
	}
	b, e := json.Marshal(result)
	if e != nil {
		return nil, e
	}
	var m map[string]any
	if e = json.Unmarshal(b, &m); e != nil {
		return nil, e
	}
	objectKey, _ := m["object_key"].(string)
	checksum, _ := hex.DecodeString(c.Checksum)
	url, e := objects.UploadURL(ctx, objectKey, c.MediaType, c.Size, base64.StdEncoding.EncodeToString(checksum))
	delete(m, "object_key")
	if e != nil {
		return nil, e
	}
	m["upload_url"] = url
	m["checksum_base64"] = base64.StdEncoding.EncodeToString(checksum)
	return m, nil
}
func (s *Service) FinalizeFile(ctx context.Context, a Actor, id string, objects Objects) error {
	if !s.IntegrationEnabled(ctx, "files") || objects == nil {
		return Disabled
	}
	number, _, e := s.DocumentAccess(ctx, s.DB, a, id, "core.documents.write")
	if e != nil {
		return e
	}
	var key, media, hash, status string
	var size int64
	if e = s.DB.QueryRowContext(ctx, "SELECT object_key,media_type,checksum,size_bytes,status FROM document_versions WHERE document_id=? AND number=?", id, number).Scan(&key, &media, &hash, &size, &status); e != nil {
		return e
	}
	if status == "quarantined" || status == "available" {
		// A lost verification acknowledgement is safe to replay after live authorization.
		return nil
	}
	if status != "pending" {
		return Conflict
	}
	info, e := objects.Info(ctx, key)
	if e != nil {
		return e
	}
	expected, _ := hex.DecodeString(hash)
	if info.Size != size || info.Checksum != base64.StdEncoding.EncodeToString(expected) || !strings.HasPrefix(http.DetectContentType(info.Prefix), media) {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		current, _, e := s.DocumentAccess(ctx, tx, a, id, "core.documents.write")
		if e != nil {
			return e
		}
		if current != number {
			return Conflict
		}
		if e = Changed(tx.ExecContext(ctx, "UPDATE document_versions SET status='quarantined' WHERE document_id=? AND number=? AND status='pending'", id, number)); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "files.verify", "document", id, "quarantined", map[string]int{"number": number})
	})
}
func (s *Service) ScanFile(ctx context.Context, a Actor, id string, number int, clean bool, reference string) error {
	if ID(id) != nil || number < 1 || len(reference) < 10 || len(reference) > 200 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.documents.scan", "", "", true); e != nil {
			return e
		}
		status := "rejected"
		if clean {
			status = "available"
		}
		if e := Changed(tx.ExecContext(ctx, "UPDATE document_versions SET status=?,scan_reference=? WHERE document_id=? AND number=? AND status='quarantined'", status, reference, id, number)); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "files.scan", "document", id, status, map[string]any{"number": number, "scanner_reference": reference})
	})
}
func (s *Service) DownloadFile(ctx context.Context, a Actor, id string, number int, objects Objects) (any, error) {
	if !s.IntegrationEnabled(ctx, "files") || objects == nil {
		return nil, Disabled
	}
	current, _, e := s.DocumentAccess(ctx, s.DB, a, id, "core.documents.read")
	if e != nil {
		return nil, e
	}
	if number == 0 {
		number = current
	}
	var key, name, status string
	if e = s.DB.QueryRowContext(ctx, "SELECT object_key,filename,status FROM document_versions WHERE document_id=? AND number=?", id, number).Scan(&key, &name, &status); e != nil {
		return nil, Missing
	}
	if status != "available" {
		return nil, Forbidden
	}
	url, e := objects.DownloadURL(ctx, key, name)
	if e != nil {
		return nil, e
	}
	if e = s.Audit(ctx, s.DB, a, "files.download", "document", id, "success", map[string]int{"number": number}); e != nil {
		return nil, e
	}
	return map[string]any{"url": url, "expires_in": int((5 * time.Minute).Seconds())}, nil
}
func (s *Service) Documents(ctx context.Context, a Actor, limit, offset int) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.identity.self", a.UserID, "", false); e != nil {
		return nil, e
	}
	grants, e := s.Grants(ctx, s.DB, a.UserID)
	if e != nil {
		return nil, e
	}
	if !hasDocumentGrant(grants) {
		return nil, Forbidden
	}
	rows, e := s.DB.QueryContext(ctx, `SELECT d.id,d.current_version,d.version,v.filename,v.media_type,v.size_bytes,v.status FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version WHERE d.deleted_at IS NULL AND d.entity_type IS NULL AND (EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='core.documents.read' AND (rp.scope='all' OR (rp.scope='self' AND d.owner_id=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=d.team_id AND tm.user_id=?))))) ORDER BY d.id DESC LIMIT ? OFFSET ?`, a.UserID, a.UserID, a.UserID, limit, offset)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []map[string]any{}
	for rows.Next() {
		var id, name, media, status string
		var number int
		var version uint64
		var size int64
		if e = rows.Scan(&id, &number, &version, &name, &media, &size, &status); e != nil {
			return nil, e
		}
		out = append(out, map[string]any{"id": id, "number": number, "version": fmt.Sprint(version), "filename": name, "media_type": media, "size_bytes": size, "status": status})
	}
	return map[string]any{"data": out}, rows.Err()
}
func hasDocumentGrant(g []Grant) bool {
	for _, v := range g {
		if v.Permission == "core.documents.read" {
			return true
		}
	}
	return false
}
