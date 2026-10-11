package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"time"
)

type FromProcessInput struct {
	Date    string `json:"date"`
	Version uint64 `json:"version,string"`
}

func (s *Service) CreateFromProcess(ctx context.Context, a core.Actor, id, key string, p FromProcessInput) (any, error) {
	if a.Type != "user" || s.Processes == nil {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version != 0 {
		return nil, core.Invalid
	}
	if d, e := time.Parse("2006-01-02", p.Date); e != nil || d.Year() < 1900 || d.Year() > 9999 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.create"); e != nil {
			return e
		}
		source, e := s.Processes.QuoteSourceInTransaction(ctx, tx, a, id)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.from_process:"+id, key, p, func() (any, error) {
			c := CustomerInput{Name: source.Entity}
			cv := uint64(0)
			if s.Customers != nil {
				match, e := s.Customers.ResolveInTransaction(ctx, tx, a, source.Entity)
				if e != nil && e != core.Forbidden {
					return nil, e
				}
				if e == nil && match != nil {
					c = snapshot(*match)
					cv = match.Version
				}
			}
			input := Input{Customer: c, Reference: source.Reference, Date: p.Date, ValidityDays: 30, Currency: "HNL", CalculationInput: CalculationInput{Items: []Item{}, DiscountType: "amount", Discount: "0"}}
			if e := normalize(&input); e != nil {
				return nil, e
			}
			r, e := Calculate(input.CalculationInput)
			if e != nil {
				return nil, e
			}
			q := Quote{Source: id, Status: "draft", Customer: c, CustomerVersion: cv, Reference: source.Reference, Date: p.Date, ValidityDays: 30, Currency: "HNL", DiscountType: "amount", Discount: "0", Result: r, Owner: a.UserID}
			v, e := s.persist(ctx, tx, a, &q, "quote.created")
			if e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "process.quote_created", "processes", id, source.Version); e != nil {
				return nil, e
			}
			return v, nil
		})
		return e
	})
	return out, e
}
func (s *Service) Clone(ctx context.Context, a core.Actor, id, key string, version uint64) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if version == 0 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		var e error
		out, e = s.cloneInTransaction(ctx, tx, a, id, key, version)
		return e
	})
	return out, e
}

// Shared by the single-record action and durable bulk item transaction.
func (s *Service) cloneInTransaction(ctx context.Context, tx *sql.Tx, a core.Actor, id, key string, version uint64) (any, error) {
	if e := s.gate(ctx, tx, a, "cotizaciones.create"); e != nil {
		return nil, e
	}
	q, e := s.access(ctx, tx, a, id, "cotizaciones.view", true, false)
	if e != nil {
		return nil, e
	}
	if e = s.items(ctx, tx, &q); e != nil {
		return nil, e
	}
	return s.Core.Idempotent(ctx, tx, a, "quotes.clone:"+id, key, map[string]any{"version": fmt.Sprint(version)}, func() (any, error) {
		if q.Deleted != nil || q.Version != version {
			return nil, core.Conflict
		}
		q.PurchaseOrderID = nil
		q.ID = ""
		q.Number = ""
		q.Status = "draft"
		q.Owner = a.UserID
		q.Team = ""
		q.Version = 1
		q.Deleted = nil
		return s.persist(ctx, tx, a, &q, "quote.cloned")
	})
}
func (s *Service) Trash(ctx context.Context, a core.Actor, id, key string, version uint64, restore bool) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if version == 0 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.delete"); e != nil {
			return e
		}
		q, e := s.access(ctx, tx, a, id, "cotizaciones.delete", true, true)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.trash:"+id, key, map[string]any{"version": fmt.Sprint(version), "restore": restore}, func() (any, error) {
			if q.Version != version || (restore != (q.Deleted != nil)) || q.Status != "draft" {
				return nil, core.Conflict
			}
			q.Version++
			topic := "quote.deleted"
			now := time.Now().UTC()
			q.Deleted = &now
			if restore {
				q.Deleted = nil
				topic = "quote.restored"
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET deleted_at=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, q.Deleted, a.UserID, id, version)); e != nil {
				return nil, e
			}
			if e = s.items(ctx, tx, &q); e != nil {
				return nil, e
			}
			return s.record(ctx, tx, a, q, topic)
		})
		return e
	})
	return out, e
}

type Filter struct {
	Search  string `json:"search"`
	Process string `json:"process_id"`
	Deleted bool   `json:"deleted"`
	Page    int    `json:"page"`
	PerPage int    `json:"per_page"`
}
type Summary struct {
	ID        string `json:"id"`
	Number    string `json:"number"`
	Name      string `json:"customer_name"`
	Reference string `json:"reference"`
	Date      string `json:"date"`
	Currency  string `json:"currency"`
	Total     string `json:"total"`
	Status    string `json:"status"`
	Version   uint64 `json:"version,string"`
}
type List struct {
	Data    []Summary `json:"data"`
	Total   int64     `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (List, error) {
	out := List{Data: []Summary{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || !validText(f.Search, 500, false) || (f.Process != "" && core.ID(f.Process) != nil) {
		return out, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "cotizaciones.view"); e != nil {
		return out, e
	}
	where := `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='cotizaciones.view' AND (rp.scope='all' OR (rp.scope='self' AND c.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.user_id=? AND tm.team_id=c.team_id))))`
	args := []any{a.UserID, a.UserID, a.UserID}
	if f.Deleted {
		where += ` AND c.deleted_at IS NOT NULL`
	} else {
		where += ` AND c.deleted_at IS NULL`
	}
	if f.Process != "" {
		where += ` AND c.source_process_id=?`
		args = append(args, f.Process)
	}
	if f.Search != "" {
		// The consecutive uses ascii_bin; compare it in the same Unicode
		// collation as the bound search so non-ASCII customer queries stay valid.
		where += ` AND (LOCATE(CONVERT(? USING utf8mb4) COLLATE utf8mb4_unicode_ci,CONVERT(c.number USING utf8mb4) COLLATE utf8mb4_unicode_ci)>0 OR LOCATE(?,c.process_reference_snapshot)>0 OR LOCATE(?,c.customer_name_snapshot)>0 OR EXISTS(SELECT 1 FROM quote_items i WHERE i.quote_id=c.id AND LOCATE(?,i.description)>0))`
		args = append(args, f.Search, f.Search, f.Search, f.Search)
	}
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM quotes c WHERE `+where, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT c.id,c.number,c.customer_name_snapshot,c.process_reference_snapshot,CAST(c.date AS CHAR),c.currency,c.total,c.status,c.version FROM quotes c WHERE `+where+` ORDER BY c.id DESC LIMIT ? OFFSET ?`, append(args, f.PerPage, (f.Page-1)*f.PerPage)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var v Summary
		if e = rows.Scan(&v.ID, &v.Number, &v.Name, &v.Reference, &v.Date, &v.Currency, &v.Total, &v.Status, &v.Version); e != nil {
			return out, e
		}
		out.Data = append(out.Data, v)
	}
	return out, rows.Err()
}
func (s *Service) Related(ctx context.Context, a core.Actor, id string, f Filter) (List, error) {
	if s.Processes == nil {
		return List{}, core.Forbidden
	}
	if e := s.Core.Transact(ctx, func(tx *sql.Tx) error { _, e := s.Processes.QuoteSourceInTransaction(ctx, tx, a, id); return e }); e != nil {
		return List{}, e
	}
	f.Process = id
	return s.List(ctx, a, f)
}
func (s *Service) Versions(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if _, e := s.Get(ctx, a, id); e != nil {
		return nil, e
	}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM quote_versions WHERE quote_id=?`, id).Scan(&total); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT data FROM quote_versions WHERE quote_id=? ORDER BY version DESC LIMIT ? OFFSET ?`, id, limit, (page-1)*limit)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	data := []Quote{}
	for rows.Next() {
		var raw string
		if e = rows.Scan(&raw); e != nil {
			return nil, e
		}
		var v Quote
		if e = json.Unmarshal([]byte(raw), &v); e != nil {
			return nil, e
		}
		data = append(data, v)
	}
	return map[string]any{"data": data, "total": total, "page": page, "per_page": limit}, rows.Err()
}
