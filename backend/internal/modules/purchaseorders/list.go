package purchaseorders

import (
	"context"
	"regexp"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
)

var listMoneyPattern = regexp.MustCompile(`^(0|[1-9][0-9]{0,15})(\.[0-9]{1,2})?$`)

type Filter struct {
	Search     string `json:"search,omitempty"`
	CustomerID string `json:"customer_id,omitempty"`
	Status     string `json:"status,omitempty"`
	TotalMin   string `json:"total_min,omitempty"`
	TotalMax   string `json:"total_max,omitempty"`
	Page       int    `json:"page"`
	PerPage    int    `json:"per_page"`
}

type CustomerSummary struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type Summary struct {
	ID          string          `json:"id"`
	OrderNumber string          `json:"order_number"`
	Customer    CustomerSummary `json:"customer"`
	OrderDate   string          `json:"order_date"`
	Currency    string          `json:"currency"`
	Total       string          `json:"total"`
	Status      string          `json:"status"`
	Version     uint64          `json:"version,string"`
}

type Page struct {
	Data    []Summary `json:"data"`
	Total   int64     `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

func listMoney(v string) (decimal.Decimal, error) {
	if !listMoneyPattern.MatchString(v) {
		return decimal.Zero, core.Invalid
	}
	d, e := decimal.NewFromString(v)
	if e != nil || d.IsNegative() {
		return decimal.Zero, core.Invalid
	}
	return d, nil
}

func orderScope(a core.Actor, permission string) (string, []any) {
	return `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission=? AND (rp.scope='all' OR (rp.scope='self' AND o.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.user_id=? AND tm.team_id=o.team_id))))`, []any{a.UserID, permission, a.UserID, a.UserID}
}

func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (Page, error) {
	out := Page{Data: []Summary{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || !finance.ValidText(f.Search, 1000, false) {
		return out, core.Invalid
	}
	if f.CustomerID != "" && core.ID(f.CustomerID) != nil {
		return out, core.Invalid
	}
	if f.Status != "" && f.Status != "received" {
		return out, core.Invalid
	}

	var min, max decimal.Decimal
	var e error
	if f.TotalMin != "" {
		if min, e = listMoney(f.TotalMin); e != nil {
			return out, e
		}
	}
	if f.TotalMax != "" {
		if max, e = listMoney(f.TotalMax); e != nil {
			return out, e
		}
	}
	if f.TotalMin != "" && f.TotalMax != "" && min.GreaterThan(max) {
		return out, core.Invalid
	}
	if e = s.gate(ctx, s.Core.DB, a, "ordenes.view"); e != nil {
		return out, e
	}

	where, args := orderScope(a, "ordenes.view")
	if f.Search != "" {
		where += ` AND (LOCATE(?,o.order_number)>0 OR LOCATE(?,o.customer_name_snapshot)>0 OR LOCATE(?,o.comments)>0 OR EXISTS(SELECT 1 FROM customer_purchase_order_items oi WHERE oi.purchase_order_id=o.id AND (LOCATE(?,oi.description)>0 OR LOCATE(?,oi.comments)>0)))`
		args = append(args, f.Search, f.Search, f.Search, f.Search, f.Search)
	}
	if f.CustomerID != "" {
		where += ` AND o.customer_id=?`
		args = append(args, f.CustomerID)
	}
	if f.Status != "" {
		where += ` AND o.status=?`
		args = append(args, f.Status)
	}
	if f.TotalMin != "" {
		where += ` AND o.total>=?`
		args = append(args, min.StringFixed(2))
	}
	if f.TotalMax != "" {
		where += ` AND o.total<=?`
		args = append(args, max.StringFixed(2))
	}

	if e = s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM customer_purchase_orders o WHERE `+where, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(o.id AS CHAR),o.order_number,COALESCE(CAST(o.customer_id AS CHAR),''),o.customer_name_snapshot,CAST(o.order_date AS CHAR),o.currency,CAST(o.total AS CHAR),o.status,o.version FROM customer_purchase_orders o WHERE `+where+` ORDER BY o.order_date DESC,o.id DESC LIMIT ? OFFSET ?`, append(args, f.PerPage, (f.Page-1)*f.PerPage)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var v Summary
		if e = rows.Scan(&v.ID, &v.OrderNumber, &v.Customer.ID, &v.Customer.Name, &v.OrderDate, &v.Currency, &v.Total, &v.Status, &v.Version); e != nil {
			return out, e
		}
		v.Total = decimal.RequireFromString(v.Total).StringFixed(2)
		out.Data = append(out.Data, v)
	}
	return out, rows.Err()
}
