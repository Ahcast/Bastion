package expenses

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"github.com/go-sql-driver/mysql"
)

type SupplierSource interface {
	SnapshotInTransaction(context.Context, *sql.Tx, core.Actor, string) (suppliers.Snapshot, error)
}
type Service struct {
	Core      *core.Service
	Suppliers SupplierSource
}

func nullable(v string) any {
	if v == "" {
		return nil
	}
	return v
}
func dbError(e error) error {
	var m *mysql.MySQLError
	if errors.As(e, &m) && (m.Number == 1062 || m.Number == 1452) {
		return core.Conflict
	}
	return e
}
func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, p string) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, p, a.UserID, team, false)
}

const columns = `CAST(e.id AS CHAR),COALESCE(e.invoice_number,''),CAST(e.expense_date AS CHAR),COALESCE(CAST(e.supplier_id AS CHAR),''),e.supplier_name_snapshot,e.supplier_version,e.category,e.currency,CAST(e.taxable_amount AS CHAR),CAST(e.exempt_amount AS CHAR),CAST(e.tax_rate AS CHAR),CAST(e.tax_amount AS CHAR),CAST(e.total AS CHAR),e.notes,e.calculator_version,e.version,CAST(e.created_by AS CHAR),CAST(e.updated_by AS CHAR),COALESCE(CAST(e.team_id AS CHAR),''),e.created_at,e.updated_at,e.deleted_at,CAST(e.deleted_by AS CHAR),e.delete_reason`

func targets(o *Expense) []any {
	return []any{&o.ID, &o.InvoiceNumber, &o.Date, &o.Supplier.ID, &o.Supplier.Name, &o.SupplierVersion, &o.Category, &o.Currency, &o.Taxable, &o.Exempt, &o.Calculation.Rate, &o.Calculation.Tax, &o.Calculation.Total, &o.Notes, &o.Calculation.CalculatorVersion, &o.Version, &o.CreatedBy, &o.UpdatedBy, &o.TeamID, &o.CreatedAt, &o.UpdatedAt, &o.DeletedAt, &o.DeletedBy, &o.DeleteReason}
}
func finalize(o *Expense) {
	o.Calculation.Currency = o.Currency
	o.Calculation.Taxable = o.Taxable
	o.Calculation.Exempt = o.Exempt
}
func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, p string, lock bool) (Expense, error) {
	var o Expense
	if core.ID(id) != nil {
		return o, core.Invalid
	}
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT `+columns+` FROM expenses e WHERE e.id=?`+suffix, id).Scan(targets(&o)...)
	if e == sql.ErrNoRows {
		return o, core.Missing
	}
	if e != nil {
		return o, e
	}
	if e = s.Core.Authorize(ctx, q, a, p, o.CreatedBy, o.TeamID, false); e != nil {
		if e == core.Forbidden {
			e = core.Missing
		}
		return Expense{}, e
	}
	if o.DeletedAt != nil && p != "gastos.delete" {
		if e = s.Core.Authorize(ctx, q, a, "gastos.restore", o.CreatedBy, o.TeamID, false); e != nil {
			return Expense{}, core.Missing
		}
	}
	finalize(&o)
	return o, nil
}
func (s *Service) Get(ctx context.Context, a core.Actor, id string) (Expense, error) {
	if e := s.gate(ctx, s.Core.DB, a, "gastos.view"); e != nil {
		return Expense{}, e
	}
	return s.access(ctx, s.Core.DB, a, id, "gastos.view", false)
}
func (s *Service) destination(ctx context.Context, tx *sql.Tx, a core.Actor, p, owner, team string) error {
	if e := s.Core.Authorize(ctx, tx, a, p, owner, team, false); e != nil {
		return e
	}
	if team != "" {
		var n int
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?`, team, a.UserID).Scan(&n); e != nil {
			return e
		}
		if n == 0 {
			if e := s.Core.Authorize(ctx, tx, a, p, "", "", false); e != nil {
				return e
			}
		}
	}
	return s.Core.Authorize(ctx, tx, a, "gastos.view", owner, team, false)
}
func (s *Service) Save(ctx context.Context, a core.Actor, id, key string, p Input) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if key == "" || id != "" && core.ID(id) != nil || id == "" && p.Version != 0 || id != "" && p.Version == 0 || p.Version == ^uint64(0) {
		return nil, core.Invalid
	}
	calc, e := Validate(p)
	if e != nil {
		return nil, e
	}
	permission := "gastos.create"
	if id != "" {
		permission = "gastos.update"
	}
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, permission); e != nil {
			return e
		}
		old := Expense{CreatedBy: a.UserID}
		if id != "" {
			var e error
			old, e = s.access(ctx, tx, a, id, permission, true)
			if e != nil {
				return e
			}
			if e = s.Core.Authorize(ctx, tx, a, "gastos.view", old.CreatedBy, old.TeamID, false); e != nil {
				return e
			}
			if old.DeletedAt != nil {
				return core.Conflict
			}
			if p.TeamID != old.TeamID {
				if e = s.Core.Authorize(ctx, tx, a, permission, old.CreatedBy, old.TeamID, true); e != nil {
					return e
				}
			}
		}
		if e := s.destination(ctx, tx, a, permission, old.CreatedBy, p.TeamID); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "expenses.save:"+id, key, p, func() (any, error) {
			if id != "" && old.Version != p.Version {
				return nil, core.Conflict
			}
			name, sv := p.Supplier.Name, uint64(0)
			if p.Supplier.ID != "" {
				if id != "" && p.Supplier.ID == old.Supplier.ID {
					name, sv = old.Supplier.Name, old.SupplierVersion
				} else {
					if s.Suppliers == nil {
						return nil, core.Forbidden
					}
					snap, e := s.Suppliers.SnapshotInTransaction(ctx, tx, a, p.Supplier.ID)
					if e != nil {
						return nil, e
					}
					name, sv = snap.Name, snap.Version
				}
			}
			version := p.Version + 1
			dest := id
			topic := "expense.updated"
			vals := []any{nullable(p.InvoiceNumber), p.Date, nullable(p.Supplier.ID), name, sv, p.Category, calc.Currency, calc.Taxable, calc.Exempt, calc.Rate, calc.Tax, calc.Total, p.Notes, calc.CalculatorVersion, version, a.UserID, nullable(p.TeamID)}
			if id == "" {
				topic = "expense.created"
				res, e := tx.ExecContext(ctx, `INSERT INTO expenses(invoice_number,expense_date,supplier_id,supplier_name_snapshot,supplier_version,category,currency,taxable_amount,exempt_amount,tax_rate,tax_amount,total,notes,calculator_version,version,updated_by,team_id,created_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, append(vals, a.UserID)...)
				if e != nil {
					return nil, e
				}
				n, e := res.LastInsertId()
				if e != nil {
					return nil, e
				}
				dest = core.WrapID(n)
			} else {
				if e := core.Changed(tx.ExecContext(ctx, `UPDATE expenses SET invoice_number=?,expense_date=?,supplier_id=?,supplier_name_snapshot=?,supplier_version=?,category=?,currency=?,taxable_amount=?,exempt_amount=?,tax_rate=?,tax_amount=?,total=?,notes=?,calculator_version=?,version=?,updated_by=?,team_id=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=? AND deleted_at IS NULL`, append(vals, id, p.Version)...)); e != nil {
					return nil, e
				}
			}
			if e := s.evidence(ctx, tx, a, topic, dest, version, ""); e != nil {
				return nil, e
			}
			return map[string]string{"id": dest, "version": fmt.Sprint(version)}, nil
		})
		if e != nil {
			return e
		}
		raw, e := json.Marshal(out)
		if e != nil {
			return e
		}
		var r struct {
			ID string `json:"id"`
		}
		if e = json.Unmarshal(raw, &r); e != nil {
			return e
		}
		dest, e := s.access(ctx, tx, a, r.ID, "gastos.view", true)
		if e != nil {
			return e
		}
		return s.Core.Authorize(ctx, tx, a, permission, dest.CreatedBy, dest.TeamID, false)
	})
	return out, dbError(e)
}
func (s *Service) evidence(ctx context.Context, tx *sql.Tx, a core.Actor, topic, id string, v uint64, reason string) error {
	if e := s.Core.Audit(ctx, tx, a, topic, "expenses", id, "success", map[string]string{"version": fmt.Sprint(v), "reason": reason}); e != nil {
		return e
	}
	return s.Core.EmitEvent(ctx, tx, a, topic, "expenses", id, v)
}
func (s *Service) State(ctx context.Context, a core.Actor, id, key string, p StateInput, restore bool) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || key == "" || p.Version == 0 || p.Version == ^uint64(0) || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	permission, topic := "gastos.delete", "expense.deleted"
	if restore {
		permission, topic = "gastos.restore", "expense.restored"
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, permission); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, permission, true)
		if e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, "gastos.view", o.CreatedBy, o.TeamID, false); e != nil {
			return e
		}
		if e = s.Core.Authorize(ctx, tx, a, permission, o.CreatedBy, o.TeamID, true); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, topic+":"+id, key, p, func() (any, error) {
			if o.Version != p.Version || (o.DeletedAt != nil) != restore {
				return nil, core.Conflict
			}
			stateSQL := `deleted_at=UTC_TIMESTAMP(6),deleted_by=?,delete_reason=?`
			args := []any{a.UserID, p.Reason}
			if restore {
				stateSQL = `deleted_at=NULL,deleted_by=NULL,delete_reason=NULL`
				args = nil
			}
			args = append(args, a.UserID, id, p.Version)
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE expenses SET `+stateSQL+`,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, args...)); e != nil {
				return nil, e
			}
			if e := s.evidence(ctx, tx, a, topic, id, p.Version+1, p.Reason); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1)}, nil
		})
		return e
	})
	return out, dbError(e)
}
