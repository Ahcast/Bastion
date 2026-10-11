package invoices

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
)

type Filter struct {
	Search, CustomerID, Status string
	Page, PerPage              int
}
type Summary struct {
	ID          string        `json:"id"`
	Number      *string       `json:"invoice_number"`
	Customer    CustomerInput `json:"customer"`
	InvoiceDate string        `json:"invoice_date"`
	Currency    string        `json:"currency"`
	Total       string        `json:"total"`
	Status      string        `json:"status"`
	Version     uint64        `json:"version,string"`
}
type Page struct {
	Data    []Summary `json:"data"`
	Total   int64     `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (Page, error) {
	out := Page{Data: []Summary{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || !finance.ValidText(f.Search, 1000, false) || f.CustomerID != "" && core.ID(f.CustomerID) != nil || f.Status != "" && f.Status != "draft" {
		return out, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "facturas.view"); e != nil {
		return out, e
	}
	where := `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='facturas.view' AND (rp.scope='all' OR (rp.scope='self' AND i.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=i.team_id AND tm.user_id=?))))`
	args := []any{a.UserID, a.UserID, a.UserID}
	if f.Search != "" {
		where += ` AND (LOCATE(?,i.customer_name_snapshot)>0 OR LOCATE(?,i.customer_rtn_snapshot)>0 OR LOCATE(?,i.public_notes)>0 OR EXISTS(SELECT 1 FROM invoice_items item WHERE item.invoice_id=i.id AND LOCATE(?,item.description)>0))`
		args = append(args, f.Search, f.Search, f.Search, f.Search)
	}
	if f.CustomerID != "" {
		where += ` AND i.customer_id=?`
		args = append(args, f.CustomerID)
	}
	if f.Status != "" {
		where += ` AND i.status=?`
		args = append(args, f.Status)
	}
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM invoices i WHERE `+where, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(i.id AS CHAR),i.invoice_number,COALESCE(CAST(i.customer_id AS CHAR),''),i.customer_name_snapshot,CAST(i.invoice_date AS CHAR),i.currency,CAST(i.total AS CHAR),i.status,i.version FROM invoices i WHERE `+where+` ORDER BY i.invoice_date DESC,i.id DESC LIMIT ? OFFSET ?`, append(args, f.PerPage, (f.Page-1)*f.PerPage)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var v Summary
		if e = rows.Scan(&v.ID, &v.Number, &v.Customer.ID, &v.Customer.Name, &v.InvoiceDate, &v.Currency, &v.Total, &v.Status, &v.Version); e != nil {
			return out, e
		}
		out.Data = append(out.Data, v)
	}
	return out, rows.Err()
}
