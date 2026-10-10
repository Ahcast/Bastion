package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

type PDFStore interface {
	PutImmutable(context.Context, string, []byte, string) error
}

func (s *Service) DocumentPolicy(ctx context.Context, q core.Querier, a core.Actor, id string, write bool) error {
	perm := "cotizaciones.view"
	if write {
		perm = "cotizaciones.update"
	}
	v, e := s.access(ctx, q, a, id, perm, write, false)
	if e != nil {
		return e
	}
	if v.Deleted != nil || (write && (v.Status != "draft" && v.Status != "costing")) {
		return core.Missing
	}
	return nil
}
func (s *Service) LinkDocument(ctx context.Context, a core.Actor, id, key, document string, version, documentVersion uint64) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if version == 0 || documentVersion == 0 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.access(ctx, tx, a, id, "cotizaciones.update", true, true)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.document:"+id, key, map[string]any{"document_id": document, "version": fmt.Sprint(version), "document_version": fmt.Sprint(documentVersion)}, func() (any, error) {
			if q.Deleted != nil || (q.Status != "draft" && q.Status != "costing") || q.Version != version {
				return nil, core.Conflict
			}
			if e := s.Core.BindDocument(ctx, tx, a, document, "quotes", id, documentVersion); e != nil {
				return nil, e
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, a.UserID, id, version)); e != nil {
				return nil, e
			}
			q.Version++
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			return s.record(ctx, tx, a, q, "quote.document_linked")
		})
		return e
	})
	return out, e
}
func (s *Service) Documents(ctx context.Context, a core.Actor, id string, page, limit int, deleted bool) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if e := s.DocumentPolicy(ctx, s.Core.DB, a, id, false); e != nil {
		return nil, e
	}
	if e := s.gate(ctx, s.Core.DB, a, "core.documents.read"); e != nil {
		return nil, e
	}
	where := `d.entity_type='quotes' AND d.entity_id=? AND d.is_generated=FALSE AND EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='core.documents.read' AND (rp.scope='all' OR (rp.scope='self' AND d.owner_id=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=d.team_id AND tm.user_id=?))))`
	if deleted {
		where += ` AND d.deleted_at IS NOT NULL`
	} else {
		where += ` AND d.deleted_at IS NULL`
	}
	args := []any{id, a.UserID, a.UserID, a.UserID}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM documents d WHERE `+where, args...).Scan(&total); e != nil {
		return nil, e
	}
	data, e := s.Core.Rows(ctx, `SELECT CAST(d.id AS CHAR) id,CAST(d.version AS CHAR) version,d.current_version,v.filename,v.media_type,v.status,v.size_bytes FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version WHERE `+where+` ORDER BY d.id DESC LIMIT ? OFFSET ?`, append(args, limit, (page-1)*limit)...)
	if e != nil {
		return nil, e
	}
	return map[string]any{"data": data.(map[string]any)["data"], "total": total, "page": page, "per_page": limit}, nil
}
func (s *Service) pdfGate(ctx context.Context, q core.Querier, a core.Actor, id string) (Quote, error) {
	v, e := s.access(ctx, q, a, id, "cotizaciones.pdf", false, false)
	if e != nil {
		return v, e
	}
	if v.Deleted != nil {
		return v, core.Missing
	}
	if e = s.Core.Authorize(ctx, q, a, "cotizaciones.view", v.Owner, v.Team, false); e != nil {
		return v, e
	}
	return v, e
}
func (s *Service) GeneratePDF(ctx context.Context, a core.Actor, id, key string, version uint64) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if version == 0 {
		return nil, core.Invalid
	}
	if s.Store == nil || !s.Core.IntegrationEnabled(ctx, "files") {
		return nil, core.Disabled
	}
	var reserved any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.access(ctx, tx, a, id, "cotizaciones.view", true, false)
		if e != nil {
			return e
		}
		if _, e = s.pdfGate(ctx, tx, a, id); e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "core.documents.write", a.UserID, "", false); e != nil {
			return e
		}
		reserved, e = s.Core.Idempotent(ctx, tx, a, "quotes.pdf:"+id, key, map[string]string{"version": fmt.Sprint(version)}, func() (any, error) {
			if q.Version != version {
				return nil, core.Conflict
			}
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			c, cv, e := s.Core.PublicCompany(ctx, tx)
			if e != nil {
				return nil, e
			}
			public := Public(q, c, cv, s.Logo, s.Stamp)
			b, e := json.Marshal(public)
			if e != nil {
				return nil, e
			}
			if len(b) > 3*1024*1024 {
				return nil, core.Invalid
			}
			hash := security.Hash(string(b))
			var snap string
			e = tx.QueryRowContext(ctx, `SELECT id FROM quote_pdf_snapshots WHERE quote_id=? AND input_hash=?`, id, hash).Scan(&snap)
			if e == sql.ErrNoRows {
				r, e := tx.ExecContext(ctx, `INSERT INTO quote_pdf_snapshots(quote_id,quote_version,input_hash,public_data,object_key,status,created_by,created_at) VALUES(?,?,?,?,?,'pending',?,UTC_TIMESTAMP(6))`, id, version, hash, string(b), "quotes/"+id+"/pdf/"+hash+".pdf", a.UserID)
				if e != nil {
					return nil, e
				}
				n, e := r.LastInsertId()
				if e != nil {
					return nil, e
				}
				snap = core.WrapID(n)
			} else if e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.pdf_reserved", "quotes", id, "pending", map[string]string{"snapshot_id": snap, "version": fmt.Sprint(version)}); e != nil {
				return nil, e
			}
			return map[string]string{"id": snap}, nil
		})
		return e
	})
	if e != nil {
		return nil, e
	}
	b, e := json.Marshal(reserved)
	if e != nil {
		return nil, e
	}
	var result map[string]string
	if e = json.Unmarshal(b, &result); e != nil {
		return nil, e
	}
	snap := result["id"]
	var raw, objectKey, status string
	var document sql.NullString
	var quoteVersion uint64
	if e = s.Core.DB.QueryRowContext(ctx, `SELECT public_data,object_key,status,document_id,quote_version FROM quote_pdf_snapshots WHERE id=? AND quote_id=?`, snap, id).Scan(&raw, &objectKey, &status, &document, &quoteVersion); e != nil {
		return nil, e
	}
	if status == "available" {
		return map[string]string{"id": snap, "document_id": document.String, "quote_version": fmt.Sprint(quoteVersion), "status": "available"}, nil
	}
	var p PublicQuote
	if e = json.Unmarshal([]byte(raw), &p); e != nil {
		return nil, e
	}
	data, _, e := RenderPDF(p)
	if e != nil {
		return nil, e
	}
	hash := security.Hash(string(data))
	if e = s.Store.PutImmutable(ctx, objectKey, data, hash); e != nil {
		return nil, e
	}
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if _, e := s.pdfGate(ctx, tx, a, id); e != nil {
			return e
		}
		if e := tx.QueryRowContext(ctx, `SELECT status,document_id FROM quote_pdf_snapshots WHERE id=? AND quote_id=? FOR UPDATE`, snap, id).Scan(&status, &document); e != nil {
			return e
		}
		if status == "available" {
			return nil
		}
		var e error
		document.String, e = s.Core.RegisterGeneratedFile(ctx, tx, a, "quotes", id, objectKey, core.FileRequest{Filename: p.Number + "-v" + fmt.Sprint(p.Version) + ".pdf", MediaType: "application/pdf", Size: int64(len(data)), Checksum: hash})
		if e != nil {
			return e
		}
		if e = core.Changed(tx.ExecContext(ctx, `UPDATE quote_pdf_snapshots SET document_id=?,status='available' WHERE id=? AND status='pending'`, document.String, snap)); e != nil {
			return e
		}
		return s.Core.Audit(ctx, tx, a, "quote.pdf_generated", "quotes", id, "success", map[string]string{"snapshot_id": snap, "document_id": document.String})
	})
	if e != nil {
		return nil, e
	}
	return map[string]string{"id": snap, "document_id": document.String, "quote_version": fmt.Sprint(quoteVersion), "status": "available"}, nil
}
func (s *Service) PDFs(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if _, e := s.pdfGate(ctx, s.Core.DB, a, id); e != nil {
		return nil, e
	}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM quote_pdf_snapshots WHERE quote_id=?`, id).Scan(&total); e != nil {
		return nil, e
	}
	data, e := s.Core.Rows(ctx, `SELECT CAST(id AS CHAR) id,CAST(quote_version AS CHAR) quote_version,CAST(document_id AS CHAR) document_id,status,created_at FROM quote_pdf_snapshots WHERE quote_id=? ORDER BY id DESC LIMIT ? OFFSET ?`, id, limit, (page-1)*limit)
	if e != nil {
		return nil, e
	}
	return map[string]any{"data": data.(map[string]any)["data"], "total": total, "page": page, "per_page": limit}, nil
}
func (s *Service) PDFDocument(ctx context.Context, a core.Actor, id, snapshot string) (string, error) {
	if core.ID(snapshot) != nil {
		return "", core.Invalid
	}
	if _, e := s.pdfGate(ctx, s.Core.DB, a, id); e != nil {
		return "", e
	}
	var document string
	e := s.Core.DB.QueryRowContext(ctx, `SELECT document_id FROM quote_pdf_snapshots WHERE id=? AND quote_id=? AND status='available'`, snapshot, id).Scan(&document)
	if e == sql.ErrNoRows {
		return "", core.Missing
	}
	return document, e
}
