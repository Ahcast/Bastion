package purchaseorders

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
)

type DocumentInput struct {
	DocumentID      string  `json:"document_id"`
	DocumentVersion uint64  `json:"document_version,string"`
	Version         uint64  `json:"version,string"`
	ItemID          *string `json:"item_id"`
}

func (s *Service) DocumentPolicy(ctx context.Context, q core.Querier, a core.Actor, id string, write bool) error {
	permission := "ordenes.view"
	if write {
		permission = "ordenes.documents.write"
	}
	if e := s.gate(ctx, q, a, permission); e != nil {
		return e
	}
	o, e := s.access(ctx, q, a, id, permission, write)
	if e != nil {
		return e
	}
	if e = s.Core.Authorize(ctx, q, a, "ordenes.view", o.CreatedBy, o.TeamID, false); e != nil {
		return e
	}
	if write && (o.Status == "invoiced" || o.Status == "closed") {
		return core.Forbidden
	}
	return nil
}

func (s *Service) LinkDocument(ctx context.Context, a core.Actor, id, key string, p DocumentInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || core.ID(p.DocumentID) != nil || p.Version == 0 || p.DocumentVersion == 0 || (p.ItemID != nil && core.ID(*p.ItemID) != nil) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.DocumentPolicy(ctx, tx, a, id, true); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "ordenes.documents.write", true)
		if e != nil {
			return e
		}
		// Replay must also reauthorize the document and recent human identity.
		var owner string
		var team sql.NullString
		if e = tx.QueryRowContext(ctx, `SELECT owner_id,team_id FROM documents WHERE id=?`, p.DocumentID).Scan(&owner, &team); e == sql.ErrNoRows {
			return core.Missing
		} else if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "core.documents.write", owner, team.String, true); e != nil {
			if e == core.Forbidden {
				return core.Missing
			}
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "purchase_orders.document:"+id, key, p, func() (any, error) {
			if o.Version != p.Version {
				return nil, core.Conflict
			}
			if p.ItemID != nil {
				var exists int
				if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM customer_purchase_order_items WHERE purchase_order_id=? AND id=?`, id, *p.ItemID).Scan(&exists); e != nil {
					return nil, e
				}
				if exists != 1 {
					return nil, core.Missing
				}
			}
			if e := s.Core.BindDocument(ctx, tx, a, p.DocumentID, "purchase_orders", id, p.DocumentVersion); e != nil {
				return nil, e
			}
			if _, e := tx.ExecContext(ctx, `INSERT INTO customer_purchase_order_documents(document_id,purchase_order_id,item_id,created_by,created_at) VALUES(?,?,?,?,UTC_TIMESTAMP(6))`, p.DocumentID, id, p.ItemID, a.UserID); e != nil {
				return nil, e
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "purchase_order.document_linked", "customer_purchase_orders", id, "success", map[string]any{"document_id": p.DocumentID, "item_id": p.ItemID, "version": fmt.Sprint(p.Version + 1)}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "purchase_order.document_linked", "customer_purchase_orders", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1)}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) Documents(ctx context.Context, a core.Actor, id, item string, page, limit int, deleted bool) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 || (item != "" && core.ID(item) != nil) {
		return nil, core.Invalid
	}
	if e := s.DocumentPolicy(ctx, s.Core.DB, a, id, false); e != nil {
		return nil, e
	}
	if e := s.gate(ctx, s.Core.DB, a, "core.documents.read"); e != nil {
		return nil, e
	}
	if item != "" {
		var n int
		if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM customer_purchase_order_items WHERE purchase_order_id=? AND id=?`, id, item).Scan(&n); e != nil {
			return nil, e
		}
		if n != 1 {
			return nil, core.Missing
		}
	}
	where := `l.purchase_order_id=? AND d.entity_type='purchase_orders' AND d.entity_id=l.purchase_order_id AND d.is_generated=FALSE AND EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='core.documents.read' AND (rp.scope='all' OR (rp.scope='self' AND d.owner_id=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=d.team_id AND tm.user_id=?))))`
	args := []any{id, a.UserID, a.UserID, a.UserID}
	if deleted {
		where += ` AND d.deleted_at IS NOT NULL`
	} else {
		where += ` AND d.deleted_at IS NULL`
	}
	if item != "" {
		where += ` AND l.item_id=?`
		args = append(args, item)
	}
	from := ` FROM customer_purchase_order_documents l JOIN documents d ON d.id=l.document_id JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version LEFT JOIN customer_purchase_order_items i ON i.id=l.item_id AND i.purchase_order_id=l.purchase_order_id WHERE `
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*)`+from+where, args...).Scan(&total); e != nil {
		return nil, e
	}
	data, e := s.Core.Rows(ctx, `SELECT CAST(d.id AS CHAR) id,CAST(d.version AS CHAR) version,CAST(l.item_id AS CHAR) item_id,i.position+1 item_position,d.current_version,v.filename,v.media_type,v.status,v.size_bytes`+from+where+` ORDER BY d.id DESC LIMIT ? OFFSET ?`, append(args, limit, (page-1)*limit)...)
	if e != nil {
		return nil, e
	}
	return map[string]any{"data": data.(map[string]any)["data"], "total": total, "page": page, "per_page": limit}, nil
}
