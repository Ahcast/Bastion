// Package reports owns authorized read projections, never commercial writes.
package reports

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"strings"
	"time"
)

var Permissions = []string{"reportes.view", "reportes.export"}

type Service struct {
	Core    *core.Service
	Sources map[string]bool
}
type Filter struct {
	From     string `json:"date_from"`
	To       string `json:"date_to"`
	Currency string `json:"currency"`
	Status   string `json:"status"`
	Page     int    `json:"page"`
	PerPage  int    `json:"per_page"`
}
type Row map[string]string
type Total struct {
	Currency string `json:"currency"`
	Count    int64  `json:"count"`
	Values   Row    `json:"values"`
}
type Page struct {
	Report  string  `json:"report"`
	Filters Filter  `json:"filters"`
	Data    []Row   `json:"data"`
	Total   int64   `json:"total"`
	Page    int     `json:"page"`
	PerPage int     `json:"per_page"`
	Totals  []Total `json:"totals"`
}
type Home struct {
	Filters Filter             `json:"filters"`
	Reports map[string][]Total `json:"reports"`
}
type column struct {
	name, expression string
	money            bool
}
type projection struct {
	table, date, permission, active string
	columns                         []column
}

func text(name, expression string) column  { return column{name, expression, false} }
func money(name, expression string) column { return column{name, expression, true} }

// Only reviewed identifiers/expressions enter SQL; HTTP values remain parameters.
func source(kind string) (projection, error) {
	switch kind {
	case "invoices":
		return projection{"invoices", "invoice_date", "facturas.view", "r.status<>'cancelled'", []column{text("id", "r.id"), text("invoice_number", "COALESCE(r.invoice_number,'')"), text("customer", "r.customer_name_snapshot"), text("invoice_date", "r.invoice_date"), text("due_date", "COALESCE(CAST(r.due_date AS CHAR),'')"), text("currency", "r.currency"), text("status", "r.status"), money("total", "r.total"), money("paid_total", "r.paid_total"), money("balance", "r.balance")}}, nil
	case "orders":
		return projection{"customer_purchase_orders", "order_date", "ordenes.view", "r.deleted_at IS NULL", []column{text("id", "r.id"), text("order_number", "r.order_number"), text("customer", "r.customer_name_snapshot"), text("destination", "r.destination"), text("order_date", "r.order_date"), text("currency", "r.currency"), text("status", "r.status"), money("subtotal", "r.base_15+r.base_18+r.exempt+r.exonerated"), money("tax_amount", "r.isv_15+r.isv_18"), money("total", "r.total"), money("products_investment", "r.products_investment"), money("additional_expenses", "r.additional_expenses"), money("investment_total", "r.investment_total"), money("profit", "r.profit")}}, nil
	case "expenses":
		return projection{"expenses", "expense_date", "gastos.view", "r.deleted_at IS NULL", []column{text("id", "r.id"), text("invoice_number", "COALESCE(r.invoice_number,'')"), text("supplier", "r.supplier_name_snapshot"), text("expense_date", "r.expense_date"), text("category", "r.category"), text("currency", "r.currency"), money("taxable_amount", "r.taxable_amount"), money("exempt_amount", "r.exempt_amount"), money("tax_amount", "r.tax_amount"), money("total", "r.total")}}, nil
	}
	return projection{}, core.Invalid
}
func validDate(v string) bool {
	t, e := time.Parse("2006-01-02", v)
	return e == nil && t.Format("2006-01-02") == v && t.Year() >= 1000
}
func validate(kind string, f Filter) error {
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || f.From != "" && !validDate(f.From) || f.To != "" && !validDate(f.To) || f.From != "" && f.To != "" && f.From > f.To || f.Currency != "" && f.Currency != "HNL" && f.Currency != "USD" {
		return core.Invalid
	}
	if f.Status != "" && (kind == "expenses" || kind == "home" || kind == "invoices" && f.Status != "draft" && f.Status != "issued" || kind == "orders" && !purchaseorders.IsOrderStatus(f.Status)) {
		return core.Invalid
	}
	return nil
}
func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, p string, sensitive bool) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT CAST(team_id AS CHAR) FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, p, a.UserID, team, sensitive)
}

// A repeatable snapshot keeps count, SUM and rows coherent during concurrent writes.
func (s *Service) snapshot(ctx context.Context, a core.Actor, fn func(*sql.Tx) error) error {
	tx, e := s.Core.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelRepeatableRead})
	if e != nil {
		return e
	}
	defer tx.Rollback()
	var id string
	if e = tx.QueryRowContext(ctx, `SELECT CAST(id AS CHAR) FROM users WHERE id=? FOR UPDATE`, a.UserID).Scan(&id); e != nil {
		if e == sql.ErrNoRows {
			return core.Unauthorized
		}
		return e
	}
	if e = s.gate(ctx, tx, a, "reportes.view", false); e != nil {
		return e
	}
	if e = fn(tx); e != nil {
		return e
	}
	return tx.Commit()
}
func scope(permission string) string {
	return `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission=? AND (rp.scope='all' OR (rp.scope='self' AND r.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=r.team_id AND tm.user_id=?))))`
}
func (s *Service) where(ctx context.Context, q core.Querier, a core.Actor, kind string, f Filter, export bool) (projection, string, []any, error) {
	p, e := source(kind)
	if e != nil {
		return p, "", nil, e
	}
	if !s.Sources[kind] {
		return p, "", nil, core.Missing
	}
	if e = s.gate(ctx, q, a, p.permission, false); e != nil {
		return p, "", nil, e
	}
	w := p.active
	args := []any{}
	perms := []string{"reportes.view", p.permission}
	if export {
		perms = append(perms, "reportes.export")
	}
	for _, permission := range perms {
		w += " AND " + scope(permission)
		args = append(args, a.UserID, permission, a.UserID, a.UserID)
	}
	for _, x := range []struct{ value, expression string }{{f.From, "r." + p.date + ">=?"}, {f.To, "r." + p.date + "<=?"}, {f.Currency, "r.currency=?"}, {f.Status, "r.status=?"}} {
		if x.value != "" {
			w += " AND " + x.expression
			args = append(args, x.value)
		}
	}
	return p, w, args, nil
}
func totals(ctx context.Context, q core.Querier, p projection, w string, args []any) ([]Total, error) {
	cols := []string{"r.currency", "COUNT(*)"}
	moneyCols := []column{}
	for _, c := range p.columns {
		if c.money {
			cols = append(cols, "CAST(SUM("+c.expression+") AS CHAR)")
			moneyCols = append(moneyCols, c)
		}
	}
	rows, e := q.QueryContext(ctx, "SELECT "+strings.Join(cols, ",")+" FROM "+p.table+" r WHERE "+w+" GROUP BY r.currency ORDER BY r.currency", args...)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Total{}
	for rows.Next() {
		v := Total{Values: Row{}}
		values := make([]string, len(moneyCols))
		dest := []any{&v.Currency, &v.Count}
		for i := range values {
			dest = append(dest, &values[i])
		}
		if e = rows.Scan(dest...); e != nil {
			return nil, e
		}
		for i, c := range moneyCols {
			v.Values[c.name] = values[i]
		}
		out = append(out, v)
	}
	return out, rows.Err()
}
func readRows(ctx context.Context, q core.Querier, p projection, w string, args []any, limit, offset int) ([]Row, error) {
	cols := []string{}
	for _, c := range p.columns {
		cols = append(cols, "CAST(("+c.expression+") AS CHAR)")
	}
	args = append(append([]any{}, args...), limit, offset)
	rows, e := q.QueryContext(ctx, "SELECT "+strings.Join(cols, ",")+" FROM "+p.table+" r WHERE "+w+" ORDER BY r."+p.date+" DESC,r.id DESC LIMIT ? OFFSET ?", args...)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Row{}
	for rows.Next() {
		v := Row{}
		values := make([]string, len(p.columns))
		dest := make([]any, len(values))
		for i := range values {
			dest[i] = &values[i]
		}
		if e = rows.Scan(dest...); e != nil {
			return nil, e
		}
		for i, c := range p.columns {
			v[c.name] = values[i]
		}
		out = append(out, v)
	}
	return out, rows.Err()
}
func (s *Service) Read(ctx context.Context, a core.Actor, kind string, f Filter) (Page, error) {
	out := Page{Report: kind, Filters: f, Page: f.Page, PerPage: f.PerPage, Data: []Row{}, Totals: []Total{}}
	if e := validate(kind, f); e != nil {
		return out, e
	}
	e := s.snapshot(ctx, a, func(tx *sql.Tx) error {
		p, w, args, e := s.where(ctx, tx, a, kind, f, false)
		if e != nil {
			return e
		}
		out.Totals, e = totals(ctx, tx, p, w, args)
		if e != nil {
			return e
		}
		for _, t := range out.Totals {
			out.Total += t.Count
		}
		out.Data, e = readRows(ctx, tx, p, w, args, f.PerPage, (f.Page-1)*f.PerPage)
		return e
	})
	return out, e
}
func (s *Service) Home(ctx context.Context, a core.Actor, f Filter) (Home, error) {
	out := Home{Filters: f, Reports: map[string][]Total{}}
	if e := validate("home", f); e != nil {
		return out, e
	}
	e := s.snapshot(ctx, a, func(tx *sql.Tx) error {
		for _, kind := range []string{"invoices", "orders", "expenses"} {
			if !s.Sources[kind] {
				continue
			}
			p, w, args, e := s.where(ctx, tx, a, kind, f, false)
			if e == core.Forbidden {
				continue
			}
			if e != nil {
				return e
			}
			out.Reports[kind], e = totals(ctx, tx, p, w, args)
			if e != nil {
				return e
			}
		}
		return nil
	})
	return out, e
}
