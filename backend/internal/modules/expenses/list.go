package expenses

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
)

type Filter struct {
	Search, SupplierID, From, To, Currency, Category string
	Deleted                                          bool
	Page, PerPage                                    int
}
type Page struct {
	Data    []Expense `json:"data"`
	Total   int64     `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

func scope(p string) string {
	return `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='` + p + `' AND (rp.scope='all' OR (rp.scope='self' AND e.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=e.team_id AND tm.user_id=?))))`
}
func (s *Service) where(ctx context.Context, a core.Actor, f Filter) (string, []any, error) {
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || !finance.ValidText(f.Search, 1000, false) || f.SupplierID != "" && core.ID(f.SupplierID) != nil || f.From != "" && !date(f.From) || f.To != "" && !date(f.To) || f.From != "" && f.To != "" && f.From > f.To || f.Currency != "" && f.Currency != "HNL" && f.Currency != "USD" || f.Category != "" && !category(f.Category) {
		return "", nil, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "gastos.view"); e != nil {
		return "", nil, e
	}
	w := scope("gastos.view")
	args := []any{a.UserID, a.UserID, a.UserID}
	if f.Deleted {
		if e := s.gate(ctx, s.Core.DB, a, "gastos.restore"); e != nil {
			return "", nil, e
		}
		w += ` AND e.deleted_at IS NOT NULL AND ` + scope("gastos.restore")
		args = append(args, a.UserID, a.UserID, a.UserID)
	} else {
		w += ` AND e.deleted_at IS NULL`
	}
	if f.Search != "" {
		w += ` AND (LOCATE(?,e.supplier_name_snapshot)>0 OR LOCATE(?,e.invoice_number)>0 OR LOCATE(?,e.notes)>0)`
		args = append(args, f.Search, f.Search, f.Search)
	}
	for _, x := range []struct{ v, sql string }{{f.SupplierID, `e.supplier_id=?`}, {f.From, `e.expense_date>=?`}, {f.To, `e.expense_date<=?`}, {f.Currency, `e.currency=?`}, {f.Category, `e.category=?`}} {
		if x.v != "" {
			w += ` AND ` + x.sql
			args = append(args, x.v)
		}
	}
	return w, args, nil
}
func (s *Service) readPage(ctx context.Context, w string, args []any, page, limit int) (Page, error) {
	out := Page{Data: []Expense{}, Page: page, PerPage: limit}
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM expenses e WHERE `+w, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT `+columns+` FROM expenses e WHERE `+w+` ORDER BY e.expense_date DESC,e.id DESC LIMIT ? OFFSET ?`, append(args, limit, (page-1)*limit)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var o Expense
		if e = rows.Scan(targets(&o)...); e != nil {
			return out, e
		}
		finalize(&o)
		out.Data = append(out.Data, o)
	}
	return out, rows.Err()
}
func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (Page, error) {
	w, args, e := s.where(ctx, a, f)
	if e != nil {
		return Page{}, e
	}
	return s.readPage(ctx, w, args, f.Page, f.PerPage)
}

type CurrencyTotal struct {
	Currency string `json:"currency"`
	Count    int64  `json:"count"`
	Taxable  string `json:"taxable_amount"`
	Exempt   string `json:"exempt_amount"`
	Tax      string `json:"tax_amount"`
	Total    string `json:"total"`
}

func (s *Service) Totals(ctx context.Context, a core.Actor, f Filter) ([]CurrencyTotal, error) {
	out := []CurrencyTotal{}
	w, args, e := s.where(ctx, a, f)
	if e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT e.currency,COUNT(*),CAST(SUM(e.taxable_amount) AS CHAR),CAST(SUM(e.exempt_amount) AS CHAR),CAST(SUM(e.tax_amount) AS CHAR),CAST(SUM(e.total) AS CHAR) FROM expenses e WHERE `+w+` GROUP BY e.currency ORDER BY e.currency`, args...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var x CurrencyTotal
		if e = rows.Scan(&x.Currency, &x.Count, &x.Taxable, &x.Exempt, &x.Tax, &x.Total); e != nil {
			return out, e
		}
		out = append(out, x)
	}
	return out, rows.Err()
}
func (s *Service) Duplicates(ctx context.Context, a core.Actor, id string, page, limit int) (Page, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return Page{}, core.Invalid
	}
	o, e := s.Get(ctx, a, id)
	if e != nil {
		return Page{}, e
	}
	if o.DeletedAt != nil {
		return Page{}, core.Conflict
	}
	if o.InvoiceNumber == "" {
		return Page{Data: []Expense{}, Page: page, PerPage: limit}, nil
	}
	w, args, e := s.where(ctx, a, Filter{Page: page, PerPage: limit})
	if e != nil {
		return Page{}, e
	}
	w += ` AND e.id<>? AND BINARY e.invoice_number=BINARY ? AND e.expense_date=? AND e.currency=? AND e.total=?`
	args = append(args, id, o.InvoiceNumber, o.Date, o.Currency, o.Calculation.Total)
	if o.Supplier.ID != "" {
		w += ` AND e.supplier_id=?`
		args = append(args, o.Supplier.ID)
	} else {
		w += ` AND e.supplier_id IS NULL AND BINARY e.supplier_name_snapshot=BINARY ?`
		args = append(args, o.Supplier.Name)
	}
	return s.readPage(ctx, w, args, page, limit)
}
