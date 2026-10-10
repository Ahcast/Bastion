package quotes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
	"github.com/shopspring/decimal"
	"net/mail"
	"strings"
	"time"
)

var Permissions = []string{"cotizaciones.view", "cotizaciones.create", "cotizaciones.update", "cotizaciones.delete", "cotizaciones.pdf", "cotizaciones.transition", "cotizaciones.request_approval", "cotizaciones.approve", "cotizaciones.revoke_approval", "cotizaciones.emit"}

type CustomerInput struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	RTN     string `json:"rtn"`
	Contact string `json:"contact"`
	Phone   string `json:"phone"`
	Email   string `json:"email"`
	Address string `json:"address"`
}
type Input struct {
	Customer      CustomerInput `json:"customer"`
	Reference     string        `json:"reference"`
	Date          string        `json:"date"`
	ValidityDays  int           `json:"validity_days"`
	Currency      string        `json:"currency"`
	Terms         string        `json:"terms"`
	Notes         string        `json:"private_notes"`
	AuthorContact string        `json:"author_contact"`
	CalculationInput
	Version uint64 `json:"version,string"`
}
type Quote struct {
	ID              string        `json:"id"`
	Number          string        `json:"number"`
	Status          string        `json:"status"`
	Source          string        `json:"source_process_id"`
	CustomerVersion uint64        `json:"customer_version,string"`
	Customer        CustomerInput `json:"customer"`
	Reference       string        `json:"reference"`
	Date            string        `json:"date"`
	ValidityDays    int           `json:"validity_days"`
	Currency        string        `json:"currency"`
	Terms           string        `json:"terms"`
	Notes           string        `json:"private_notes"`
	AuthorContact   string        `json:"author_contact"`
	Exoneration     string        `json:"exoneration_reference"`
	DiscountType    string        `json:"general_discount_type"`
	Discount        string        `json:"general_discount"`
	Result
	Version uint64     `json:"version,string"`
	Owner   string     `json:"created_by"`
	Team    string     `json:"team_id"`
	Deleted *time.Time `json:"deleted_at"`
}
type CustomerSource interface {
	SnapshotInTransaction(context.Context, *sql.Tx, core.Actor, string) (customers.Snapshot, error)
	ResolveInTransaction(context.Context, *sql.Tx, core.Actor, string) (*customers.Snapshot, error)
}
type ProcessSource interface {
	QuoteSourceInTransaction(context.Context, *sql.Tx, core.Actor, string) (processes.QuoteSource, error)
}
type Service struct {
	Core        *core.Service
	Approvals   *workflow.Service
	Customers   CustomerSource
	Processes   ProcessSource
	Store       PDFStore
	Logo, Stamp []byte
}

func normalize(p *Input) error {
	p.Items = append([]Item{}, p.Items...)
	if !validText(p.Customer.Name, 1000, true) || !validText(p.Customer.RTN, 30, false) || !validText(p.Customer.Contact, 120, false) || !validText(p.Customer.Phone, 50, false) || !validText(p.Customer.Address, 1000, false) || !validText(p.Reference, 500, false) || !validText(p.Terms, 12000, false) || !validText(p.Notes, 12000, false) || !validText(p.AuthorContact, 1000, false) {
		return core.Invalid
	}
	if p.Customer.ID != "" && core.ID(p.Customer.ID) != nil {
		return core.Invalid
	}
	if _, e := customers.CanonicalRTN(p.Customer.RTN); e != nil {
		return e
	}
	if p.Customer.Email != "" {
		m, e := mail.ParseAddress(p.Customer.Email)
		if e != nil || m.Address != p.Customer.Email || len(p.Customer.Email) > 254 || strings.ContainsAny(p.Customer.Email, "\r\n") {
			return core.Invalid
		}
	}
	if d, e := time.Parse("2006-01-02", p.Date); e != nil || d.Year() < 1900 || d.Year() > 9999 {
		return core.Invalid
	}
	if p.ValidityDays < 1 || p.ValidityDays > 365 || (p.Currency != "HNL" && p.Currency != "USD") {
		return core.Invalid
	}
	r, e := Calculate(p.CalculationInput)
	if e != nil {
		return e
	}
	for i, x := range r.Items {
		p.Items[i] = x.Item
	}
	d, _ := number(p.Discount, 2)
	if p.DiscountType == "percent" {
		d, _ = number(p.Discount, 4)
	}
	p.Discount = d.String()
	return nil
}
func nullable(v string) any {
	if v == "" {
		return nil
	}
	return v
}
func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, perm string) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, perm, a.UserID, team, false)
}

const columns = `id,number,status,COALESCE(CAST(source_process_id AS CHAR),''),COALESCE(CAST(customer_id AS CHAR),''),customer_version,customer_name_snapshot,customer_rtn_snapshot,customer_contact_snapshot,customer_phone_snapshot,customer_email_snapshot,customer_address_snapshot,process_reference_snapshot,CAST(date AS CHAR),validity_days,currency,terms,private_notes,author_contact,exoneration_reference,general_discount_type,general_discount,gross,item_discount,allocated_discount,base_15,base_18,exempt,exonerated,isv_15,isv_18,total,cost,margin,calculator_version,version,created_by,COALESCE(CAST(team_id AS CHAR),''),deleted_at`

func scan(row interface{ Scan(...any) error }) (Quote, error) {
	var q Quote
	q.Items = []CalculatedItem{}
	c := &q.Calculation
	e := row.Scan(&q.ID, &q.Number, &q.Status, &q.Source, &q.Customer.ID, &q.CustomerVersion, &q.Customer.Name, &q.Customer.RTN, &q.Customer.Contact, &q.Customer.Phone, &q.Customer.Email, &q.Customer.Address, &q.Reference, &q.Date, &q.ValidityDays, &q.Currency, &q.Terms, &q.Notes, &q.AuthorContact, &q.Exoneration, &q.DiscountType, &q.Discount, &c.Gross, &c.ItemDiscount, &c.GeneralDiscount, &c.Base15, &c.Base18, &c.Exempt, &c.Exonerated, &c.ISV15, &c.ISV18, &c.Total, &c.Cost, &c.Margin, &c.CalculatorVersion, &q.Version, &q.Owner, &q.Team, &q.Deleted)
	if e == nil {
		q.Discount = decimal.RequireFromString(q.Discount).String()
	}
	return q, e
}
func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, perm string, lock, sensitive bool) (Quote, error) {
	if core.ID(id) != nil {
		return Quote{}, core.Invalid
	}
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	v, e := scan(q.QueryRowContext(ctx, `SELECT `+columns+` FROM quotes WHERE id=?`+suffix, id))
	if e == sql.ErrNoRows {
		return v, core.Missing
	}
	if e != nil {
		return v, e
	}
	if e = s.Core.Authorize(ctx, q, a, perm, v.Owner, v.Team, sensitive); e == core.Forbidden {
		return v, core.Missing
	}
	return v, e
}
func (s *Service) items(ctx context.Context, q core.Querier, v *Quote) error {
	rows, e := q.QueryContext(ctx, `SELECT description,unit,quantity,unit_price,unit_cost,discount_type,discount,tax,unit_discount,gross,item_discount,allocated_discount,base,cost FROM quote_items WHERE quote_id=? ORDER BY position`, v.ID)
	if e != nil {
		return e
	}
	defer rows.Close()
	for rows.Next() {
		var x CalculatedItem
		if e = rows.Scan(&x.Description, &x.Unit, &x.Quantity, &x.Price, &x.Cost, &x.DiscountType, &x.Discount, &x.Tax, &x.UnitDiscount, &x.Gross, &x.ItemDiscount, &x.AllocatedDiscount, &x.Base, &x.LineCost); e != nil {
			return e
		}
		x.Quantity = decimal.RequireFromString(x.Quantity).String()
		x.Price = decimal.RequireFromString(x.Price).String()
		x.Cost = decimal.RequireFromString(x.Cost).String()
		x.Discount = decimal.RequireFromString(x.Discount).String()
		x.UnitDiscount = decimal.RequireFromString(x.UnitDiscount).String()
		v.Items = append(v.Items, x)
	}
	return rows.Err()
}
func (s *Service) Get(ctx context.Context, a core.Actor, id string) (Quote, error) {
	var v Quote
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "cotizaciones.view"); e != nil {
			return e
		}
		var e error
		v, e = s.access(ctx, tx, a, id, "cotizaciones.view", true, false)
		if e != nil {
			return e
		}
		return s.items(ctx, tx, &v)
	})
	return v, e
}
func (q Quote) input() Input {
	items := []Item{}
	for _, x := range q.Items {
		items = append(items, x.Item)
	}
	return Input{q.Customer, q.Reference, q.Date, q.ValidityDays, q.Currency, q.Terms, q.Notes, q.AuthorContact, CalculationInput{items, q.DiscountType, q.Discount, q.Exoneration}, q.Version}
}
func snapshot(c customers.Snapshot) CustomerInput {
	return CustomerInput{c.ID, c.Name, c.RTN, c.Contact, c.Phone, c.Email, c.Address}
}
func (s *Service) customer(ctx context.Context, tx *sql.Tx, a core.Actor, p Input, old Quote) (CustomerInput, uint64, error) {
	if p.Customer.ID == "" {
		return p.Customer, 0, nil
	}
	if old.ID != "" && p.Customer.ID == old.Customer.ID {
		return old.Customer, old.CustomerVersion, nil
	}
	if s.Customers == nil {
		return CustomerInput{}, 0, core.Forbidden
	}
	c, e := s.Customers.SnapshotInTransaction(ctx, tx, a, p.Customer.ID)
	return snapshot(c), c.Version, e
}
func (s *Service) Save(ctx context.Context, a core.Actor, id, key string, p Input) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if e := normalize(&p); e != nil {
		return nil, e
	}
	if (id == "" && p.Version != 0) || (id != "" && (core.ID(id) != nil || p.Version == 0)) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		perm := "cotizaciones.create"
		if id != "" {
			perm = "cotizaciones.update"
		}
		if e := s.gate(ctx, tx, a, perm); e != nil {
			return e
		}
		var old Quote
		if id != "" {
			var e error
			old, e = s.access(ctx, tx, a, id, perm, true, false)
			if e != nil {
				return e
			}
			if e = s.Core.Authorize(ctx, tx, a, "cotizaciones.view", old.Owner, old.Team, false); e != nil {
				return e
			}
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.save:"+id, key, p, func() (any, error) {
			if id != "" && (old.Deleted != nil || (!editableStatus(old.Status) && old.Status != "approved") || old.Version != p.Version) {
				return nil, core.Conflict
			}
			c, cv, e := s.customer(ctx, tx, a, p, old)
			if e != nil {
				return nil, e
			}
			r, e := Calculate(p.CalculationInput)
			if e != nil {
				return nil, e
			}
			q := Quote{ID: id, Number: old.Number, Status: "draft", Source: old.Source, CustomerVersion: cv, Customer: c, Reference: p.Reference, Date: p.Date, ValidityDays: p.ValidityDays, Currency: p.Currency, Terms: p.Terms, Notes: p.Notes, AuthorContact: p.AuthorContact, Exoneration: p.Exoneration, DiscountType: p.DiscountType, Discount: p.Discount, Result: r, Version: old.Version + 1, Owner: old.Owner, Team: old.Team}
			topic := "quote.updated"
			if id != "" && old.Status != "draft" {
				topic = "quote.revised"
			}
			if id == "" {
				q.Owner = a.UserID
				topic = "quote.created"
			}
			return s.persist(ctx, tx, a, &q, topic)
		})
		return e
	})
	return out, e
}
func (s *Service) persist(ctx context.Context, tx *sql.Tx, a core.Actor, q *Quote, topic string) (any, error) {
	c := q.Calculation
	if q.ID == "" {
		var n uint64
		if e := tx.QueryRowContext(ctx, `SELECT next_value FROM document_counters WHERE name='quote' FOR UPDATE`).Scan(&n); e != nil {
			return nil, e
		}
		if n == 0 || n >= 999999999999999999 {
			return nil, core.Conflict
		}
		if _, e := tx.ExecContext(ctx, `UPDATE document_counters SET next_value=next_value+1 WHERE name='quote'`); e != nil {
			return nil, e
		}
		q.Number = fmt.Sprintf("COT-%06d", n)
		q.Version = 1
		res, e := tx.ExecContext(ctx, `INSERT INTO quotes(number,status,source_process_id,customer_id,customer_version,customer_name_snapshot,customer_rtn_snapshot,customer_contact_snapshot,customer_phone_snapshot,customer_email_snapshot,customer_address_snapshot,process_reference_snapshot,date,validity_days,currency,terms,private_notes,author_contact,exoneration_reference,general_discount_type,general_discount,gross,item_discount,allocated_discount,base_15,base_18,exempt,exonerated,isv_15,isv_18,total,cost,margin,calculator_version,created_by,updated_by,team_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, q.Number, q.Status, nullable(q.Source), nullable(q.Customer.ID), q.CustomerVersion, q.Customer.Name, q.Customer.RTN, q.Customer.Contact, q.Customer.Phone, q.Customer.Email, q.Customer.Address, q.Reference, q.Date, q.ValidityDays, q.Currency, q.Terms, q.Notes, q.AuthorContact, q.Exoneration, q.DiscountType, q.Discount, c.Gross, c.ItemDiscount, c.GeneralDiscount, c.Base15, c.Base18, c.Exempt, c.Exonerated, c.ISV15, c.ISV18, c.Total, c.Cost, c.Margin, c.CalculatorVersion, q.Owner, a.UserID, nullable(q.Team))
		if e != nil {
			return nil, e
		}
		insertID, e := res.LastInsertId()
		if e != nil {
			return nil, e
		}
		q.ID = core.WrapID(insertID)
	} else {
		if e := core.Changed(tx.ExecContext(ctx, `UPDATE quotes SET status=?,customer_id=?,customer_version=?,customer_name_snapshot=?,customer_rtn_snapshot=?,customer_contact_snapshot=?,customer_phone_snapshot=?,customer_email_snapshot=?,customer_address_snapshot=?,process_reference_snapshot=?,date=?,validity_days=?,currency=?,terms=?,private_notes=?,author_contact=?,exoneration_reference=?,general_discount_type=?,general_discount=?,gross=?,item_discount=?,allocated_discount=?,base_15=?,base_18=?,exempt=?,exonerated=?,isv_15=?,isv_18=?,total=?,cost=?,margin=?,calculator_version=?,version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, q.Status, nullable(q.Customer.ID), q.CustomerVersion, q.Customer.Name, q.Customer.RTN, q.Customer.Contact, q.Customer.Phone, q.Customer.Email, q.Customer.Address, q.Reference, q.Date, q.ValidityDays, q.Currency, q.Terms, q.Notes, q.AuthorContact, q.Exoneration, q.DiscountType, q.Discount, c.Gross, c.ItemDiscount, c.GeneralDiscount, c.Base15, c.Base18, c.Exempt, c.Exonerated, c.ISV15, c.ISV18, c.Total, c.Cost, c.Margin, c.CalculatorVersion, a.UserID, q.ID, q.Version-1)); e != nil {
			return nil, e
		}
		if _, e := tx.ExecContext(ctx, `DELETE FROM quote_items WHERE quote_id=?`, q.ID); e != nil {
			return nil, e
		}
	}
	for i, x := range q.Items {
		if _, e := tx.ExecContext(ctx, `INSERT INTO quote_items VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)`, q.ID, i, x.Description, x.Unit, x.Quantity, x.Price, x.Cost, x.DiscountType, x.Discount, x.Tax, x.UnitDiscount, x.Gross, x.ItemDiscount, x.AllocatedDiscount, x.Base, x.LineCost); e != nil {
			return nil, e
		}
	}
	return s.record(ctx, tx, a, *q, topic)
}
func (s *Service) record(ctx context.Context, tx *sql.Tx, a core.Actor, q Quote, topic string) (any, error) {
	if s.Approvals != nil && topic != "quote.approved" {
		if e := s.Approvals.Invalidate(ctx, tx, a, "quotes", q.ID, topic); e != nil {
			return nil, e
		}
	}
	data, e := json.Marshal(q)
	if e != nil {
		return nil, e
	}
	if _, e = tx.ExecContext(ctx, `INSERT INTO quote_versions VALUES(?,?,?,?,UTC_TIMESTAMP(6))`, q.ID, q.Version, string(data), a.UserID); e != nil {
		return nil, e
	}
	if e = s.Core.Audit(ctx, tx, a, topic, "quotes", q.ID, "success", map[string]string{"version": fmt.Sprint(q.Version)}); e != nil {
		return nil, e
	}
	if e = s.Core.EmitEvent(ctx, tx, a, topic, "quotes", q.ID, q.Version); e != nil {
		return nil, e
	}
	return map[string]string{"id": q.ID, "version": fmt.Sprint(q.Version)}, nil
}
