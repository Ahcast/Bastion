package suppliers

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"strings"
	"time"
)

type PriceInput struct {
	SourceURL    string    `json:"source_url"`
	SourceType   string    `json:"source_type"`
	ObservedAt   time.Time `json:"source_observed_at"`
	PartNumber   string    `json:"part_number"`
	Price        string    `json:"observed_price"`
	Currency     string    `json:"currency"`
	Availability string    `json:"availability"`
	LeadTime     string    `json:"lead_time"`
	MOQ          string    `json:"moq"`
	Taxes        string    `json:"taxes"`
	Shipping     string    `json:"shipping"`
	Version      uint64    `json:"version,string"`
}
type Observation struct {
	ID           string    `json:"id"`
	SourceURL    string    `json:"source_url"`
	SourceType   string    `json:"source_type"`
	ObservedAt   time.Time `json:"source_observed_at"`
	PartNumber   string    `json:"part_number"`
	Price        string    `json:"observed_price"`
	Currency     string    `json:"currency"`
	Availability string    `json:"availability"`
	LeadTime     string    `json:"lead_time"`
	MOQ          string    `json:"moq"`
	Taxes        string    `json:"taxes"`
	Shipping     string    `json:"shipping"`
	UserName     string    `json:"user_name"`
	CapturedAt   time.Time `json:"captured_at"`
}
type PricePage struct {
	Data    []Observation `json:"data"`
	Total   int64         `json:"total"`
	Page    int           `json:"page"`
	PerPage int           `json:"per_page"`
}

func ValidatePrice(p *PriceInput, now time.Time) error {
	if p.Version == 0 || !website(p.SourceURL, false) || (p.SourceType != "web" && p.SourceType != "catalog") || p.ObservedAt.IsZero() || p.ObservedAt.Year() < 1970 || p.ObservedAt.After(now) || !text(p.PartNumber, 80, true) || !text(p.Availability, 500, true) || !text(p.LeadTime, 120, true) {
		return core.Invalid
	}
	p.ObservedAt = p.ObservedAt.UTC().Truncate(time.Microsecond)
	p.PartNumber = strings.TrimSpace(p.PartNumber)
	p.Currency = strings.ToUpper(p.Currency)
	v, e := Decimal(p.Price, 18, 6)
	if e != nil {
		return e
	}
	p.Price = v
	v, e = Decimal(p.MOQ, 18, 4)
	if e != nil {
		return e
	}
	p.MOQ = v
	if p.Price == "" {
		if p.Currency != "" {
			return core.Invalid
		}
	} else {
		if len(p.Currency) != 3 {
			return core.Invalid
		}
		for _, r := range p.Currency {
			if r < 'A' || r > 'Z' {
				return core.Invalid
			}
		}
	}
	for _, v := range []string{p.Taxes, p.Shipping} {
		if v != "unknown" && v != "included" && v != "excluded" {
			return core.Invalid
		}
	}
	return nil
}
func (s *Service) RecordPrice(ctx context.Context, a core.Actor, id, key string, p PriceInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if e := ValidatePrice(&p, time.Now()); e != nil {
		return nil, e
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "proveedores.update"); e != nil {
			return e
		}
		c, e := s.access(ctx, tx, a, id, "proveedores.update", true, false)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "suppliers.price:"+id, key, p, func() (any, error) {
			if c.Version != p.Version {
				return nil, core.Conflict
			}
			r, e := tx.ExecContext(ctx, `INSERT INTO supplier_price_observations(supplier_id,source_url,source_type,source_observed_at,part_number,observed_price,currency,availability,lead_time,moq,taxes,shipping,user_id,captured_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6))`, id, p.SourceURL, p.SourceType, p.ObservedAt, p.PartNumber, nullable(p.Price), p.Currency, p.Availability, p.LeadTime, nullable(p.MOQ), p.Taxes, p.Shipping, a.UserID)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE suppliers SET version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, a.UserID, id, p.Version)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "suppliers.price_observed", "suppliers", id, "success", map[string]string{"observation_id": core.WrapID(n), "version": fmt.Sprint(p.Version + 1), "source_type": p.SourceType}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "suppliers.price_observed", "suppliers", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": core.WrapID(n), "version": fmt.Sprint(p.Version + 1)}, nil
		})
		return e
	})
	return out, e
}
func (s *Service) PriceHistory(ctx context.Context, a core.Actor, id, part string, page, limit int) (PricePage, error) {
	out := PricePage{Data: []Observation{}, Page: page, PerPage: limit}
	if page < 1 || page > 100000 || limit < 1 || limit > 100 || !text(part, 80, true) {
		return out, core.Invalid
	}
	if _, e := s.Get(ctx, a, id); e != nil {
		return out, e
	}
	predicate := `p.supplier_id=?`
	args := []any{id}
	if part != "" {
		predicate += ` AND BINARY p.part_number=?`
		args = append(args, part)
	}
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM supplier_price_observations p WHERE `+predicate, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT p.id,p.source_url,p.source_type,p.source_observed_at,p.part_number,COALESCE(CAST(p.observed_price AS CHAR),''),p.currency,p.availability,p.lead_time,COALESCE(CAST(p.moq AS CHAR),''),p.taxes,p.shipping,u.name,p.captured_at FROM supplier_price_observations p JOIN users u ON u.id=p.user_id WHERE `+predicate+` ORDER BY p.id DESC LIMIT ? OFFSET ?`, append(args, limit, (page-1)*limit)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var p Observation
		if e = rows.Scan(&p.ID, &p.SourceURL, &p.SourceType, &p.ObservedAt, &p.PartNumber, &p.Price, &p.Currency, &p.Availability, &p.LeadTime, &p.MOQ, &p.Taxes, &p.Shipping, &p.UserName, &p.CapturedAt); e != nil {
			return out, e
		}
		out.Data = append(out.Data, p)
	}
	return out, rows.Err()
}
