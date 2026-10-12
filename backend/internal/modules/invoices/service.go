package invoices

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/go-sql-driver/mysql"
	"github.com/shopspring/decimal"
	"strings"
	"time"
)

type CustomerSource interface {
	SnapshotInTransaction(context.Context, *sql.Tx, core.Actor, string) (customers.Snapshot, error)
}
type Service struct {
	Core      *core.Service
	Customers CustomerSource
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
func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, permission string) error {
	return s.gateSensitive(ctx, q, a, permission, false)
}
func (s *Service) gateSensitive(ctx context.Context, q core.Querier, a core.Actor, permission string, sensitive bool) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, permission, a.UserID, team, sensitive)
}

var financialColumns = []string{"gross", "item_discount", "allocated_discount", "base_15", "base_18", "exempt", "exonerated", "isv_15", "isv_18", "total", "products_investment", "additional_expenses", "investment_total", "profit"}

func financialValues(c Financials) []any {
	return []any{c.Gross, c.ItemDiscount, c.GeneralDiscount, c.Base15, c.Base18, c.Exempt, c.Exonerated, c.ISV15, c.ISV18, c.Total, c.ProductsInvestment, c.AdditionalExpenses, c.InvestmentTotal, c.Profit}
}
func financialTargets(c *Financials) []any {
	return []any{&c.Gross, &c.ItemDiscount, &c.GeneralDiscount, &c.Base15, &c.Base18, &c.Exempt, &c.Exonerated, &c.ISV15, &c.ISV18, &c.Total, &c.ProductsInvestment, &c.AdditionalExpenses, &c.InvestmentTotal, &c.Profit}
}
func castColumns(prefix string, columns []string) string {
	out := make([]string, len(columns))
	for i, c := range columns {
		out[i] = "CAST(" + prefix + c + " AS CHAR)"
	}
	return strings.Join(out, ",")
}

const headerColumns = `i.id,i.invoice_number,CAST(i.sar_snapshot_json AS CHAR),i.status,COALESCE(CAST(i.customer_id AS CHAR),''),i.customer_version,i.customer_name_snapshot,i.customer_rtn_snapshot,i.customer_contact_snapshot,i.customer_phone_snapshot,i.customer_email_snapshot,i.customer_address_snapshot,CAST(i.invoice_date AS CHAR),i.payment_term_type,i.credit_days,i.currency,i.public_notes,i.exoneration_reference,i.general_discount_type,CAST(i.general_discount AS CHAR)`

func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, permission string, lock bool) (Invoice, error) {
	var o Invoice
	var sar, company sql.NullString
	o.Items = []Item{}
	if core.ID(id) != nil {
		return o, core.Invalid
	}
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	targets := []any{&o.ID, &o.Number, &sar, &o.Status, &o.Customer.ID, &o.CustomerVersion, &o.Customer.Name, &o.Customer.RTN, &o.Customer.Contact, &o.Customer.Phone, &o.Customer.Email, &o.Customer.Address, &o.InvoiceDate, &o.PaymentTerm, &o.CreditDays, &o.Currency, &o.PublicNotes, &o.Exoneration, &o.DiscountType, &o.Discount}
	targets = append(targets, financialTargets(&o.Calculation)...)
	targets = append(targets, &o.PaidTotal, &o.Balance, &o.Calculation.CalculatorVersion, &o.Calculation.ProfitVersion, &o.Version, &o.CreatedBy, &o.TeamID, &o.IssuedAt, &o.IssuedBy, &o.DueDate, &company, &o.CompanyVersion, &o.SourceQuoteID, &o.SourceQuoteVersion)
	e := q.QueryRowContext(ctx, `SELECT `+headerColumns+`,`+castColumns("i.", financialColumns)+`,CAST(i.paid_total AS CHAR),CAST(i.balance AS CHAR),i.calculator_version,i.profit_version,i.version,CAST(i.created_by AS CHAR),COALESCE(CAST(i.team_id AS CHAR),''),i.issued_at,CAST(i.issued_by AS CHAR),CAST(i.due_date AS CHAR),CAST(i.company_snapshot_json AS CHAR),i.company_version,CAST(i.source_quote_id AS CHAR),i.source_quote_version FROM invoices i WHERE i.id=?`+suffix, id).Scan(targets...)
	if e == sql.ErrNoRows {
		return Invoice{}, core.Missing
	}
	if e != nil {
		return Invoice{}, e
	}
	if e = s.Core.Authorize(ctx, q, a, permission, o.CreatedBy, o.TeamID, false); e != nil {
		if e == core.Forbidden {
			return Invoice{}, core.Missing
		}
		return Invoice{}, e
	}
	if company.Valid {
		o.Company = json.RawMessage(company.String)
	}
	if sar.Valid {
		o.SAR = json.RawMessage(sar.String)
	}
	o.Discount = decimal.RequireFromString(o.Discount).String()
	return o, nil
}
func (s *Service) items(ctx context.Context, q core.Querier, o *Invoice) error {
	rows, e := q.QueryContext(ctx, `SELECT CAST(id AS CHAR),description,unit,CAST(quantity AS CHAR),CAST(unit_price AS CHAR),CAST(unit_cost AS CHAR),discount_type,CAST(discount AS CHAR),tax_type,CAST(unit_discount AS CHAR),CAST(gross AS CHAR),CAST(item_discount AS CHAR),CAST(allocated_discount AS CHAR),CAST(base AS CHAR),CAST(line_cost AS CHAR) FROM invoice_items WHERE invoice_id=? ORDER BY position`, o.ID)
	if e != nil {
		return e
	}
	defer rows.Close()
	for rows.Next() {
		var x Item
		if e = rows.Scan(&x.ID, &x.Description, &x.Unit, &x.Quantity, &x.Price, &x.Cost, &x.DiscountType, &x.Discount, &x.Tax, &x.UnitDiscount, &x.Gross, &x.ItemDiscount, &x.AllocatedDiscount, &x.Base, &x.LineCost); e != nil {
			return e
		}
		x.Quantity = decimal.RequireFromString(x.Quantity).String()
		x.Price = decimal.RequireFromString(x.Price).String()
		x.Cost = decimal.RequireFromString(x.Cost).String()
		x.Discount = decimal.RequireFromString(x.Discount).String()
		x.UnitDiscount = decimal.RequireFromString(x.UnitDiscount).String()
		o.Items = append(o.Items, x)
	}
	return rows.Err()
}
func (s *Service) Get(ctx context.Context, a core.Actor, id string) (Invoice, error) {
	var out Invoice
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "facturas.view"); e != nil {
			return e
		}
		var e error
		out, e = s.access(ctx, tx, a, id, "facturas.view", true)
		if e != nil {
			return e
		}
		if e = s.items(ctx, tx, &out); e != nil {
			return e
		}
		return invoiceCommercial(&out, time.Now().UTC())
	})
	return out, e
}
func (s *Service) destination(ctx context.Context, tx *sql.Tx, a core.Actor, permission, owner, team string) error {
	if e := s.Core.Authorize(ctx, tx, a, permission, owner, team, false); e != nil {
		return e
	}
	if team != "" {
		var n int
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?`, team, a.UserID).Scan(&n); e != nil {
			return e
		}
		if n == 0 {
			if e := s.Core.Authorize(ctx, tx, a, permission, "", "", false); e != nil {
				return e
			}
		}
	}
	return s.Core.Authorize(ctx, tx, a, "facturas.view", owner, team, false)
}
func (s *Service) customer(ctx context.Context, tx *sql.Tx, a core.Actor, p CustomerInput, old Invoice) (CustomerInput, uint64, error) {
	if p.ID == "" {
		return p, 0, nil
	}
	if old.ID != "" && p.ID == old.Customer.ID {
		return old.Customer, old.CustomerVersion, nil
	}
	if s.Customers == nil {
		return CustomerInput{}, 0, core.Forbidden
	}
	c, e := s.Customers.SnapshotInTransaction(ctx, tx, a, p.ID)
	if e != nil {
		return CustomerInput{}, 0, e
	}
	return CustomerInput{ID: c.ID, Name: c.Name, RTN: c.RTN, Contact: c.Contact, Phone: c.Phone, Email: c.Email, Address: c.Address}, c.Version, nil
}
func (s *Service) Save(ctx context.Context, a core.Actor, id, key string, p Input) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	calculated, e := Validate(p)
	if e != nil {
		return nil, e
	}
	if id == "" && p.Version != 0 || id != "" && (core.ID(id) != nil || p.Version == 0) {
		return nil, core.Invalid
	}
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		permission := "facturas.create"
		if id != "" {
			permission = "facturas.update"
		}
		if e := s.gate(ctx, tx, a, permission); e != nil {
			return e
		}
		var old Invoice
		if id != "" {
			var e error
			old, e = s.access(ctx, tx, a, id, permission, true)
			if e != nil {
				return e
			}
			if e = s.Core.Authorize(ctx, tx, a, "facturas.view", old.CreatedBy, old.TeamID, false); e != nil {
				if e == core.Forbidden {
					return core.Missing
				}
				return e
			}
			if e = s.items(ctx, tx, &old); e != nil {
				return e
			}
			if p.TeamID != old.TeamID {
				if e = s.Core.Authorize(ctx, tx, a, permission, "", "", true); e != nil {
					return e
				}
				if e = s.destination(ctx, tx, a, permission, old.CreatedBy, p.TeamID); e != nil {
					return e
				}
			}
		} else {
			if e := s.destination(ctx, tx, a, permission, a.UserID, p.TeamID); e != nil {
				return e
			}
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "invoices.save:"+id, key, p, func() (any, error) {
			if id != "" && (old.Version != p.Version || old.Status != "draft") {
				return nil, core.Conflict
			}
			owned := map[string]bool{}
			for _, x := range old.Items {
				owned[x.ID] = true
			}
			for _, x := range p.Items {
				if x.ID != "" && !owned[x.ID] {
					return nil, core.Invalid
				}
			}
			customer, version, e := s.customer(ctx, tx, a, p.Customer, old)
			if e != nil {
				return nil, e
			}
			o := Invoice{ID: id, Status: "draft", Customer: customer, CustomerVersion: version, InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, CreditDays: p.CreditDays, Currency: p.Currency, PublicNotes: p.PublicNotes, DiscountType: p.DiscountType, Discount: p.Discount, Exoneration: p.Exoneration, Calculation: calculated.Calculation, PaidTotal: "0.00", Balance: calculated.Calculation.Total, Version: old.Version + 1, CreatedBy: old.CreatedBy, TeamID: p.TeamID}
			if id == "" {
				o.CreatedBy = a.UserID
				o.Version = 1
			}
			for i, x := range calculated.Items {
				o.Items = append(o.Items, Item{ID: p.Items[i].ID, CalculatedItem: x})
			}
			return s.persist(ctx, tx, a, &o)
		})
		if e != nil {
			return e
		}
		raw, e := json.Marshal(out)
		if e != nil {
			return e
		}
		var result struct {
			ID string `json:"id"`
		}
		if e = json.Unmarshal(raw, &result); e != nil {
			return e
		}
		_, e = s.access(ctx, tx, a, result.ID, "facturas.view", true)
		return e
	})
	return out, dbError(e)
}
func placeholders(n int) string { return strings.TrimSuffix(strings.Repeat("?,", n), ",") }
func (s *Service) persist(ctx context.Context, tx *sql.Tx, a core.Actor, o *Invoice) (any, error) {
	cols := []string{"status", "customer_id", "customer_version", "customer_name_snapshot", "customer_rtn_snapshot", "customer_contact_snapshot", "customer_phone_snapshot", "customer_email_snapshot", "customer_address_snapshot", "invoice_date", "payment_term_type", "credit_days", "currency", "public_notes", "exoneration_reference", "general_discount_type", "general_discount"}
	vals := []any{o.Status, nullable(o.Customer.ID), o.CustomerVersion, o.Customer.Name, o.Customer.RTN, o.Customer.Contact, o.Customer.Phone, o.Customer.Email, o.Customer.Address, o.InvoiceDate, o.PaymentTerm, o.CreditDays, o.Currency, o.PublicNotes, o.Exoneration, o.DiscountType, o.Discount}
	cols = append(cols, financialColumns...)
	vals = append(vals, financialValues(o.Calculation)...)
	cols = append(cols, "paid_total", "balance", "calculator_version", "profit_version", "version", "updated_by", "team_id")
	vals = append(vals, o.PaidTotal, o.Balance, o.Calculation.CalculatorVersion, o.Calculation.ProfitVersion, o.Version, a.UserID, nullable(o.TeamID))
	topic := "invoice.created"
	if o.ID == "" {
		cols = append(cols, "created_by", "source_quote_id", "source_quote_version")
		vals = append(vals, o.CreatedBy, o.SourceQuoteID, o.SourceQuoteVersion)
		res, e := tx.ExecContext(ctx, `INSERT INTO invoices(`+strings.Join(cols, ",")+`,created_at,updated_at) VALUES(`+placeholders(len(vals))+`,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, vals...)
		if e != nil {
			return nil, e
		}
		id, e := res.LastInsertId()
		if e != nil {
			return nil, e
		}
		o.ID = core.WrapID(id)
	} else {
		topic = "invoice.updated"
		sets := make([]string, len(cols))
		for i, c := range cols {
			sets[i] = c + "=?"
		}
		if e := core.Changed(tx.ExecContext(ctx, `UPDATE invoices SET `+strings.Join(sets, ",")+`,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, append(vals, o.ID, o.Version-1)...)); e != nil {
			return nil, e
		}
	}
	if _, e := tx.ExecContext(ctx, `UPDATE invoice_items SET position=position+1000 WHERE invoice_id=?`, o.ID); e != nil {
		return nil, e
	}
	kept := []any{o.ID}
	for pos, x := range o.Items {
		cols := []string{"position", "description", "unit", "quantity", "unit_price", "unit_cost", "discount_type", "discount", "tax_type", "unit_discount", "gross", "item_discount", "allocated_discount", "base", "line_cost"}
		vals := []any{pos + 1, x.Description, x.Unit, x.Quantity, x.Price, x.Cost, x.DiscountType, x.Discount, x.Tax, x.UnitDiscount, x.Gross, x.ItemDiscount, x.AllocatedDiscount, x.Base, x.LineCost}
		if x.ID == "" {
			vals = append(vals, o.ID)
			res, e := tx.ExecContext(ctx, `INSERT INTO invoice_items(`+strings.Join(cols, ",")+`,invoice_id) VALUES(`+placeholders(len(vals))+`)`, vals...)
			if e != nil {
				return nil, e
			}
			id, e := res.LastInsertId()
			if e != nil {
				return nil, e
			}
			x.ID = core.WrapID(id)
			o.Items[pos].ID = x.ID
		} else {
			sets := make([]string, len(cols))
			for i, c := range cols {
				sets[i] = c + "=?"
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE invoice_items SET `+strings.Join(sets, ",")+` WHERE id=? AND invoice_id=?`, append(vals, x.ID, o.ID)...)); e != nil {
				return nil, e
			}
		}
		kept = append(kept, x.ID)
	}
	if _, e := tx.ExecContext(ctx, `DELETE FROM invoice_items WHERE invoice_id=? AND id NOT IN (`+placeholders(len(kept)-1)+`)`, kept...); e != nil {
		return nil, e
	}
	if e := s.Core.Audit(ctx, tx, a, topic, "invoices", o.ID, "success", map[string]string{"version": fmt.Sprint(o.Version), "status": "draft"}); e != nil {
		return nil, e
	}
	if e := s.Core.EmitEvent(ctx, tx, a, topic, "invoices", o.ID, o.Version); e != nil {
		return nil, e
	}
	return map[string]string{"id": o.ID, "version": fmt.Sprint(o.Version)}, nil
}
