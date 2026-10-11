package invoices

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/go-sql-driver/mysql"
)

type FiscalSettingsInput struct {
	Version uint64              `json:"version,string"`
	Value   FiscalAuthorization `json:"value"`
	Reason  string              `json:"reason"`
}
type FiscalSettings struct {
	Value     FiscalAuthorization `json:"value"`
	Version   uint64              `json:"version,string"`
	NextValue uint64              `json:"next_value,string"`
}

func fiscalSettings(ctx context.Context, tx *sql.Tx) (FiscalSettings, error) {
	var p FiscalSettings
	var raw string
	e := tx.QueryRowContext(ctx, `SELECT value,version FROM invoice_fiscal_settings WHERE id=1 FOR UPDATE`).Scan(&raw, &p.Version)
	if e == sql.ErrNoRows {
		return p, core.Missing
	}
	if e != nil {
		return p, e
	}
	if e = json.Unmarshal([]byte(raw), &p.Value); e != nil {
		return p, e
	}
	if e = ValidateFiscalAuthorization(p.Value); e != nil {
		return p, e
	}
	e = tx.QueryRowContext(ctx, `SELECT next_value FROM document_counters WHERE name='invoice' FOR UPDATE`).Scan(&p.NextValue)
	return p, e
}
func (s *Service) FiscalSettings(ctx context.Context, a core.Actor) (FiscalSettings, error) {
	var out FiscalSettings
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Core.Authorize(ctx, tx, a, "facturas.fiscal.read", "", "", false); e != nil {
			return e
		}
		var e error
		out, e = fiscalSettings(ctx, tx)
		return e
	})
	return out, e
}
func (s *Service) SaveFiscalSettings(ctx context.Context, a core.Actor, key string, p FiscalSettingsInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if key == "" || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	if e := ValidateFiscalAuthorization(p.Value); e != nil {
		return nil, e
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Core.Authorize(ctx, tx, a, "facturas.fiscal.manage", "", "", true); e != nil {
			return e
		}
		if e := s.Core.Authorize(ctx, tx, a, "facturas.fiscal.read", "", "", false); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "invoices.fiscal-settings", key, p, func() (any, error) {
			old, e := fiscalSettings(ctx, tx)
			create := e == core.Missing
			if e != nil && !create {
				return nil, e
			}
			if create && p.Version != 0 || !create && p.Version != old.Version {
				return nil, core.Conflict
			}
			next := old.NextValue
			if create {
				next = p.Value.RangeStart
			}
			if p.Value.Enabled && (next < p.Value.RangeStart || next > p.Value.RangeEnd) {
				return nil, core.Conflict
			}
			if !create && (old.Value.Prefix != p.Value.Prefix || old.Value.Width != p.Value.Width) {
				var n int
				if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM invoices WHERE invoice_number IS NOT NULL`).Scan(&n); e != nil {
					return nil, e
				}
				if n != 0 {
					return nil, core.Conflict
				}
			}
			raw, e := json.Marshal(p.Value)
			if e != nil {
				return nil, e
			}
			version := old.Version + 1
			topic := "invoice.fiscal_settings.updated"
			if version == 0 {
				return nil, core.Conflict
			}
			if create {
				topic = "invoice.fiscal_settings.created"
				if _, e = tx.ExecContext(ctx, `INSERT INTO invoice_fiscal_settings(id,value,version,updated_by,updated_at) VALUES(1,?,1,?,UTC_TIMESTAMP(6))`, string(raw), a.UserID); e != nil {
					return nil, e
				}
				if _, e = tx.ExecContext(ctx, `INSERT INTO document_counters(name,next_value) VALUES('invoice',?)`, next); e != nil {
					return nil, e
				}
			} else {
				if e = core.Changed(tx.ExecContext(ctx, `UPDATE invoice_fiscal_settings SET value=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=1 AND version=?`, string(raw), a.UserID, p.Version)); e != nil {
					return nil, e
				}
			}
			if e = s.Core.Audit(ctx, tx, a, topic, "invoice_fiscal_settings", "1", "success", map[string]string{"version": fmt.Sprint(version), "reason": p.Reason}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, topic, "invoice_fiscal_settings", "1", version); e != nil {
				return nil, e
			}
			return map[string]string{"version": fmt.Sprint(version), "next_value": fmt.Sprint(next)}, nil
		})
		return e
	})
	var db *mysql.MySQLError
	if errors.As(e, &db) && (db.Number == 1213 || db.Number == 1205) {
		e = core.Conflict
	}
	return out, dbError(e)
}
