package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

type EmitInput struct {
	Version    uint64 `json:"version,string"`
	ApprovalID string `json:"approval_id"`
}
type emission struct {
	ID, Document, Status, Public, ObjectKey, BindingHash, ApprovalID string
	Version                                                          uint64
}

func (v emission) result() map[string]string {
	return map[string]string{"id": v.ID, "document_id": v.Document, "quote_version": fmt.Sprint(v.Version), "status": v.Status, "kind": "issued", "approval_id": v.ApprovalID}
}
func loadEmission(ctx context.Context, tx *sql.Tx, id, quote string) (emission, error) {
	var v emission
	e := tx.QueryRowContext(ctx, `SELECT id,COALESCE(CAST(document_id AS CHAR),''),status,public_data,object_key,binding_hash,approval_id,quote_version FROM quote_pdf_snapshots WHERE id=? AND quote_id=? AND kind='issued' FOR UPDATE`, id, quote).Scan(&v.ID, &v.Document, &v.Status, &v.Public, &v.ObjectKey, &v.BindingHash, &v.ApprovalID, &v.Version)
	if e == sql.ErrNoRows {
		return v, core.Missing
	}
	return v, e
}
func (s *Service) emissionGate(ctx context.Context, tx *sql.Tx, a core.Actor, id string) (Quote, error) {
	q, e := s.access(ctx, tx, a, id, "cotizaciones.emit", true, true)
	if e != nil {
		return q, e
	}
	if q.Deleted != nil {
		return q, core.Missing
	}
	if _, e = s.pdfGate(ctx, tx, a, id); e != nil {
		return q, e
	}
	if e = s.Core.Authorize(ctx, tx, a, "core.documents.write", a.UserID, "", false); e != nil {
		return q, e
	}
	if !s.Core.IntegrationEnabledUsing(ctx, tx, "files") {
		return q, core.Disabled
	}
	return q, nil
}
func (s *Service) emissionBinding(ctx context.Context, tx *sql.Tx, a core.Actor, q Quote, p EmitInput) (workflow.Binding, error) {
	var b workflow.Binding
	if q.Status != "approved" || q.Version != p.Version {
		return b, core.Conflict
	}
	// Always load a fresh list: reservation may already have loaded this copy.
	q.Items = nil
	if e := s.items(ctx, tx, &q); e != nil {
		return b, e
	}
	b, e := s.ApprovalBinding(ctx, tx, a, q)
	if e != nil {
		return b, e
	}
	approval, e := s.Approvals.Get(ctx, tx, p.ApprovalID, "quotes", q.ID)
	if e != nil {
		return b, e
	}
	return b, s.Approvals.Verify(ctx, tx, approval, b)
}

// Emit freezes approved evidence. Delivery is a separate business operation.
// Only the final transaction consumes approval and publishes document metadata.
func (s *Service) Emit(ctx context.Context, a core.Actor, id, key string, p EmitInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if p.Version == 0 || core.ID(p.ApprovalID) != nil {
		return nil, core.Invalid
	}
	if s.Approvals == nil || s.Store == nil {
		return nil, core.Disabled
	}
	var v emission
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.emissionGate(ctx, tx, a, id)
		if e != nil {
			return e
		}
		reserved, e := s.Core.Idempotent(ctx, tx, a, "quotes.emit:"+id, key, p, func() (any, error) {
			var snap string
			e := tx.QueryRowContext(ctx, `SELECT id FROM quote_pdf_snapshots WHERE approval_id=? AND quote_id=? AND quote_version=? AND kind='issued' FOR UPDATE`, p.ApprovalID, id, p.Version).Scan(&snap)
			if e == nil {
				return map[string]string{"id": snap}, nil
			}
			if e != sql.ErrNoRows {
				return nil, e
			}
			b, e := s.emissionBinding(ctx, tx, a, q, p)
			if e != nil {
				return nil, e
			}
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			company, cv, e := s.Core.PublicCompany(ctx, tx)
			if e != nil {
				return nil, e
			}
			raw, e := json.Marshal(Public(q, company, cv, s.Logo, s.Stamp))
			if e != nil {
				return nil, e
			}
			if len(raw) > 3*1024*1024 {
				return nil, core.Invalid
			}
			// Distinguish issued bytes from a draft with the same public projection.
			hash := security.Hash("issued:v1:" + string(raw))
			r, e := tx.ExecContext(ctx, `INSERT INTO quote_pdf_snapshots(quote_id,quote_version,input_hash,public_data,object_key,status,created_by,created_at,kind,approval_id,binding_hash) VALUES(?,?,?,?,?,'pending',?,UTC_TIMESTAMP(6),'issued',?,?)`, id, p.Version, hash, string(raw), "quotes/"+id+"/issued/"+hash+".pdf", a.UserID, p.ApprovalID, b.Hash)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			snap = core.WrapID(n)
			if e = s.Core.Audit(ctx, tx, a, "quote.emission_reserved", "quotes", id, "pending", map[string]string{"snapshot_id": snap, "approval_id": p.ApprovalID, "version": fmt.Sprint(p.Version)}); e != nil {
				return nil, e
			}
			return map[string]string{"id": snap}, nil
		})
		if e != nil {
			return e
		}
		raw, e := json.Marshal(reserved)
		if e != nil {
			return e
		}
		var result map[string]string
		if e = json.Unmarshal(raw, &result); e != nil {
			return e
		}
		v, e = loadEmission(ctx, tx, result["id"], id)
		if e != nil {
			return e
		}
		if v.ApprovalID != p.ApprovalID || v.Version != p.Version {
			return core.Conflict
		}
		if v.Status == "available" {
			return nil
		}
		b, e := s.emissionBinding(ctx, tx, a, q, p)
		if e != nil {
			return e
		}
		if b.Hash != v.BindingHash {
			return core.Conflict
		}
		return nil
	})
	if e != nil {
		return nil, e
	}
	if v.Status == "available" {
		return v.result(), nil
	}
	var public PublicQuote
	if e = json.Unmarshal([]byte(v.Public), &public); e != nil {
		return nil, e
	}
	data, _, e := RenderIssuedPDF(public)
	if e != nil {
		return nil, e
	}
	checksum := security.Hash(string(data))
	if e = s.Store.PutImmutable(ctx, v.ObjectKey, data, checksum); e != nil {
		return nil, e
	}
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		q, e := s.emissionGate(ctx, tx, a, id)
		if e != nil {
			return e
		}
		v, e = loadEmission(ctx, tx, v.ID, id)
		if e != nil {
			return e
		}
		if v.Status == "available" {
			return nil
		}
		b, e := s.emissionBinding(ctx, tx, a, q, p)
		if e != nil {
			return e
		}
		if b.Hash != v.BindingHash {
			return core.Conflict
		}
		if e = s.Approvals.Consume(ctx, tx, a, p.ApprovalID, b, "cotizaciones.emit"); e != nil {
			return e
		}
		v.Document, e = s.Core.RegisterGeneratedFile(ctx, tx, a, "quotes", id, v.ObjectKey, core.FileRequest{Filename: public.Number + "-v" + fmt.Sprint(public.Version) + "-emitida.pdf", MediaType: "application/pdf", Size: int64(len(data)), Checksum: checksum})
		if e != nil {
			return e
		}
		if e = core.Changed(tx.ExecContext(ctx, `UPDATE quote_pdf_snapshots SET document_id=?,status='available',issued_by=?,issued_at=UTC_TIMESTAMP(6) WHERE id=? AND status='pending' AND kind='issued'`, v.Document, a.UserID, v.ID)); e != nil {
			return e
		}
		if e = s.Core.Audit(ctx, tx, a, "quote.issued", "quotes", id, "success", map[string]string{"snapshot_id": v.ID, "document_id": v.Document, "approval_id": p.ApprovalID, "version": fmt.Sprint(p.Version)}); e != nil {
			return e
		}
		if e = s.Core.EmitEvent(ctx, tx, a, "quote.issued", "quotes", id, p.Version); e != nil {
			return e
		}
		v.Status = "available"
		return nil
	})
	if e != nil {
		return nil, e
	}
	return v.result(), nil
}
