package suppliers

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/go-sql-driver/mysql"
	"golang.org/x/text/unicode/norm"
	"net/mail"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"
)

var Permissions = []string{"proveedores.view", "proveedores.create", "proveedores.update"}

type Service struct{ Core *core.Service }
type Contact struct {
	Name  string `json:"name"`
	Role  string `json:"role"`
	Phone string `json:"phone"`
	Email string `json:"email"`
}
type Product struct {
	Name        string `json:"name"`
	Brand       string `json:"brand"`
	PartNumber  string `json:"part_number"`
	Description string `json:"description"`
	LeadTime    string `json:"lead_time"`
	MOQ         string `json:"moq"`
}
type Input struct {
	Name          string    `json:"name"`
	RTN           string    `json:"rtn"`
	ContactName   string    `json:"contact_name"`
	Phone         string    `json:"phone"`
	Email         string    `json:"email"`
	Address       string    `json:"address"`
	Country       string    `json:"country"`
	Website       string    `json:"website"`
	Notes         string    `json:"notes"`
	PaymentTerms  string    `json:"payment_terms"`
	LeadTime      string    `json:"lead_time"`
	Evaluation    *int      `json:"evaluation"`
	Reliability   string    `json:"reliability"`
	ResponseHours *int      `json:"response_hours"`
	Categories    []string  `json:"categories"`
	Brands        []string  `json:"brands"`
	Active        bool      `json:"is_active"`
	TeamID        string    `json:"team_id"`
	Version       uint64    `json:"version,string"`
	Contacts      []Contact `json:"contacts"`
	Products      []Product `json:"products"`
}
type Supplier struct {
	ID string `json:"id"`
	Input
	CreatedBy string    `json:"created_by"`
	UpdatedBy string    `json:"updated_by"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}
type Snapshot struct {
	ID      *string `json:"supplier_id"`
	Name    string  `json:"supplier_name_snapshot"`
	RTN     string  `json:"supplier_rtn_snapshot"`
	Contact string  `json:"supplier_contact_snapshot"`
	Phone   string  `json:"supplier_phone_snapshot"`
	Email   string  `json:"supplier_email_snapshot"`
	Address string  `json:"supplier_address_snapshot"`
	Country string  `json:"supplier_country_snapshot"`
	Website string  `json:"supplier_website_snapshot"`
	Version uint64  `json:"supplier_version,string"`
}
type Filter struct {
	Search  string `json:"search,omitempty"`
	Country string `json:"country,omitempty"`
	Active  *bool  `json:"is_active,omitempty"`
	Page    int    `json:"page"`
	PerPage int    `json:"per_page"`
}
type List struct {
	Data    []Summary `json:"data"`
	Total   int64     `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

// List results are summaries; only Get returns a complete editable aggregate.
// Omitted collections must never be confused with empty persisted collections.
type Summary struct {
	Country string `json:"country"`
	Website string `json:"website"`
	ID      string `json:"id"`
	Name    string `json:"name"`
	RTN     string `json:"rtn"`
	Email   string `json:"email"`
	Active  bool   `json:"is_active"`
	Version uint64 `json:"version,string"`
}
type Note struct {
	ID        string    `json:"id"`
	Body      string    `json:"body"`
	UserName  string    `json:"user_name"`
	CreatedAt time.Time `json:"created_at"`
}

func CanonicalName(v string) string {
	v = norm.NFKD.String(v)
	v = strings.Map(func(r rune) rune {
		if unicode.Is(unicode.Mn, r) {
			return -1
		}
		return unicode.ToLower(r)
	}, v)
	return strings.Join(strings.Fields(v), " ")
}
func CanonicalRTN(v string) (string, error) {
	v = strings.ReplaceAll(strings.ReplaceAll(strings.TrimSpace(v), "-", ""), " ", "")
	if v == "" {
		return "", nil
	}
	if len(v) != 14 {
		return "", core.Invalid
	}
	for _, r := range v {
		if r < '0' || r > '9' {
			return "", core.Invalid
		}
	}
	return v, nil
}
func text(v string, max int, empty bool) bool {
	return utf8.ValidString(v) && utf8.RuneCountInString(v) <= max && (empty || strings.TrimSpace(v) != "") && strings.IndexFunc(v, func(r rune) bool { return unicode.IsControl(r) && r != '\n' && r != '\t' }) < 0
}
func email(v string) bool {
	if v == "" {
		return true
	}
	m, e := mail.ParseAddress(v)
	return e == nil && m.Address == v && len(v) <= 254 && !strings.ContainsAny(v, "\r\n")
}
func Validate(p *Input) error {
	p.Contacts = append([]Contact{}, p.Contacts...)
	p.Products = append([]Product{}, p.Products...)
	p.Categories = append([]string{}, p.Categories...)
	p.Brands = append([]string{}, p.Brands...)
	if strings.ContainsAny(p.Email, "\r\n") {
		return core.Invalid
	}
	p.Name = strings.Join(strings.Fields(p.Name), " ")
	p.Country = strings.Join(strings.Fields(p.Country), " ")
	p.RTN = strings.TrimSpace(p.RTN)
	p.Email = strings.ToLower(strings.TrimSpace(p.Email))
	if !text(p.Name, 180, false) || !text(CanonicalName(p.Name), 180, false) || !text(p.Country, 80, true) || !text(CanonicalName(p.Country), 80, true) || !text(p.RTN, 30, true) || !text(p.ContactName, 120, true) || !text(p.Phone, 50, true) || !email(p.Email) || !text(p.Address, 1000, true) || !text(p.Notes, 4000, true) || !text(p.PaymentTerms, 1000, true) || !text(p.LeadTime, 120, true) || !website(p.Website, true) {
		return core.Invalid
	}
	if _, e := CanonicalRTN(p.RTN); e != nil {
		return e
	}
	if p.Reliability != "unknown" && p.Reliability != "low" && p.Reliability != "medium" && p.Reliability != "high" {
		return core.Invalid
	}
	if p.Evaluation != nil && (*p.Evaluation < 1 || *p.Evaluation > 5) {
		return core.Invalid
	}
	if p.ResponseHours != nil && (*p.ResponseHours < 0 || *p.ResponseHours > 8760) {
		return core.Invalid
	}
	if p.TeamID != "" && core.ID(p.TeamID) != nil {
		return core.Invalid
	}
	if len(p.Contacts) > 20 || len(p.Products) > 50 {
		return core.Invalid
	}
	for i := range p.Contacts {
		c := &p.Contacts[i]
		if strings.ContainsAny(c.Email, "\r\n") {
			return core.Invalid
		}
		c.Email = strings.ToLower(strings.TrimSpace(c.Email))
		if !text(c.Name, 120, false) || !text(c.Role, 100, true) || !text(c.Phone, 50, true) || !email(c.Email) {
			return core.Invalid
		}
	}
	for i := range p.Products {
		c := &p.Products[i]
		if !text(c.Name, 120, false) || !text(c.Brand, 80, true) || !text(c.PartNumber, 80, true) || !text(c.Description, 1000, true) || !text(c.LeadTime, 120, true) {
			return core.Invalid
		}
		v, e := Decimal(c.MOQ, 18, 4)
		if e != nil {
			return e
		}
		c.MOQ = v
	}
	for _, tags := range [][]string{p.Categories, p.Brands} {
		if len(tags) > 20 {
			return core.Invalid
		}
		seen := map[string]bool{}
		for i, v := range tags {
			tags[i] = strings.Join(strings.Fields(v), " ")
			canon := CanonicalName(tags[i])
			if !text(tags[i], 80, false) || !text(canon, 80, false) || seen[canon] {
				return core.Invalid
			}
			seen[canon] = true
		}
	}
	return nil
}
func dbError(e error) error {
	var m *mysql.MySQLError
	if errors.As(e, &m) && (m.Number == 1062 || m.Number == 1452) {
		return core.Conflict
	}
	return e
}
func nullable(v string) any {
	if v == "" {
		return nil
	}
	return v
}

// Gate validates live identity and at least one scope; SQL still filters each row.
func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, permission string) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, permission, a.UserID, team, false)
}
func scope(a core.Actor, permission string) (string, []any) {
	return `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission=? AND (rp.scope='all' OR (rp.scope='self' AND c.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.user_id=? AND tm.team_id=c.team_id))))`, []any{a.UserID, permission, a.UserID, a.UserID}
}

const columns = `c.id,c.name,c.rtn,c.contact_name,c.phone,c.email,c.address,c.country,c.website,c.notes,c.payment_terms,c.lead_time,c.evaluation,c.reliability,c.response_hours,c.is_active,COALESCE(CAST(c.team_id AS CHAR),''),c.version,c.created_by,c.updated_by,c.created_at,c.updated_at`

type scanner interface{ Scan(...any) error }

func scan(row scanner) (Supplier, error) {
	var c Supplier
	c.Contacts = []Contact{}
	c.Products = []Product{}
	c.Categories = []string{}
	c.Brands = []string{}
	e := row.Scan(&c.ID, &c.Name, &c.RTN, &c.ContactName, &c.Phone, &c.Email, &c.Address, &c.Country, &c.Website, &c.Notes, &c.PaymentTerms, &c.LeadTime, &c.Evaluation, &c.Reliability, &c.ResponseHours, &c.Active, &c.TeamID, &c.Version, &c.CreatedBy, &c.UpdatedBy, &c.CreatedAt, &c.UpdatedAt)
	return c, e
}
func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, permission string, lock, sensitive bool) (Supplier, error) {
	if core.ID(id) != nil {
		return Supplier{}, core.Invalid
	}
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	c, e := scan(q.QueryRowContext(ctx, `SELECT `+columns+` FROM suppliers c WHERE c.id=?`+suffix, id))
	if e == sql.ErrNoRows {
		return c, core.Missing
	}
	if e != nil {
		return c, e
	}
	if e = s.Core.Authorize(ctx, q, a, permission, c.CreatedBy, c.TeamID, sensitive); e != nil {
		if e == core.Forbidden {
			return c, core.Missing
		}
		return c, e
	}
	return c, nil
}
func (s *Service) children(ctx context.Context, q core.Querier, c *Supplier) error {
	rows, e := q.QueryContext(ctx, `SELECT name,role,phone,email FROM supplier_contacts WHERE supplier_id=? ORDER BY position`, c.ID)
	if e != nil {
		return e
	}
	for rows.Next() {
		var x Contact
		if e = rows.Scan(&x.Name, &x.Role, &x.Phone, &x.Email); e != nil {
			rows.Close()
			return e
		}
		c.Contacts = append(c.Contacts, x)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return e
	}
	rows, e = q.QueryContext(ctx, `SELECT name,brand,part_number,description,lead_time,COALESCE(CAST(moq AS CHAR),'') FROM supplier_products WHERE supplier_id=? ORDER BY position`, c.ID)
	if e != nil {
		return e
	}
	for rows.Next() {
		var x Product
		if e = rows.Scan(&x.Name, &x.Brand, &x.PartNumber, &x.Description, &x.LeadTime, &x.MOQ); e != nil {
			rows.Close()
			return e
		}
		c.Products = append(c.Products, x)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return e
	}
	rows, e = q.QueryContext(ctx, `SELECT kind,name FROM supplier_tags WHERE supplier_id=? ORDER BY kind,position`, c.ID)
	if e != nil {
		return e
	}
	defer rows.Close()
	for rows.Next() {
		var kind, v string
		if e = rows.Scan(&kind, &v); e != nil {
			return e
		}
		if kind == "category" {
			c.Categories = append(c.Categories, v)
		} else {
			c.Brands = append(c.Brands, v)
		}
	}
	return rows.Err()
}
func (s *Service) Get(ctx context.Context, a core.Actor, id string) (Supplier, error) {
	var c Supplier
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "proveedores.view"); e != nil {
			return e
		}
		var e error
		c, e = s.access(ctx, tx, a, id, "proveedores.view", true, false)
		if e != nil {
			return e
		}
		return s.children(ctx, tx, &c)
	})
	return c, e
}
func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (List, error) {
	out := List{Data: []Summary{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || !text(f.Search, 180, true) {
		return out, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "proveedores.view"); e != nil {
		return out, e
	}
	where, args := scope(a, "proveedores.view")
	if f.Search != "" {
		where += ` AND (LOCATE(?,c.canonical_name)>0 OR LOCATE(?,c.rtn)>0 OR LOCATE(?,c.email)>0)`
		args = append(args, CanonicalName(f.Search), f.Search, strings.ToLower(f.Search))
	}
	if !text(f.Country, 80, true) {
		return out, core.Invalid
	}
	if f.Country != "" {
		where += ` AND c.canonical_country=?`
		args = append(args, CanonicalName(f.Country))
	}
	if f.Active != nil {
		where += ` AND c.is_active=?`
		args = append(args, *f.Active)
	}
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM suppliers c WHERE `+where, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT `+columns+` FROM suppliers c WHERE `+where+` ORDER BY c.canonical_name,c.id LIMIT ? OFFSET ?`, append(args, f.PerPage, (f.Page-1)*f.PerPage)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		c, e := scan(rows)
		if e != nil {
			return out, e
		}
		out.Data = append(out.Data, Summary{Country: c.Country, Website: c.Website, ID: c.ID, Name: c.Name, RTN: c.RTN, Email: c.Email, Active: c.Active, Version: c.Version})
	}
	return out, rows.Err()
}
func (s *Service) Save(ctx context.Context, a core.Actor, id, key string, p Input) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if e := Validate(&p); e != nil {
		return nil, e
	}
	if id != "" && (core.ID(id) != nil || p.Version == 0) {
		return nil, core.Invalid
	}
	if id == "" && p.Version != 0 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		permission := "proveedores.create"
		if id != "" {
			permission = "proveedores.update"
		}
		if e := s.gate(ctx, tx, a, permission); e != nil {
			return e
		}
		var old Supplier
		if id != "" {
			var e error
			old, e = s.access(ctx, tx, a, id, permission, true, false)
			if e != nil {
				return e
			}
		}
		if e := s.Core.Authorize(ctx, tx, a, permission, a.UserID, p.TeamID, false); id == "" && e != nil {
			return e
		}
		if id == "" && p.TeamID != "" {
			var member int
			if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?`, p.TeamID, a.UserID).Scan(&member); e != nil {
				return e
			}
			if member == 0 {
				if e := s.Core.Authorize(ctx, tx, a, permission, "", "", false); e != nil {
					return e
				}
			}
		}
		if id != "" && p.TeamID != old.TeamID {
			if e := s.Core.Authorize(ctx, tx, a, permission, "", "", true); e != nil {
				return e
			}
		}
		if (id != "" && p.Active != old.Active) || (id == "" && !p.Active) {
			owner := old.CreatedBy
			if id == "" {
				owner = a.UserID
			}
			if e := s.Core.Authorize(ctx, tx, a, permission, owner, p.TeamID, true); e != nil {
				return e
			}
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "suppliers.save:"+id, key, p, func() (any, error) {
			rtn, _ := CanonicalRTN(p.RTN)
			newID := id
			version := uint64(1)
			topic := "suppliers.created"
			if id == "" {
				r, e := tx.ExecContext(ctx, `INSERT INTO suppliers(name,canonical_name,rtn,canonical_rtn,contact_name,phone,email,address,country,canonical_country,website,notes,payment_terms,lead_time,evaluation,reliability,response_hours,is_active,created_by,updated_by,team_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, p.Name, CanonicalName(p.Name), p.RTN, nullable(rtn), p.ContactName, p.Phone, p.Email, p.Address, p.Country, CanonicalName(p.Country), p.Website, p.Notes, p.PaymentTerms, p.LeadTime, p.Evaluation, p.Reliability, p.ResponseHours, p.Active, a.UserID, a.UserID, nullable(p.TeamID))
				if e != nil {
					return nil, dbError(e)
				}
				n, e := r.LastInsertId()
				if e != nil {
					return nil, e
				}
				newID = core.WrapID(n)
			} else {
				if old.Version != p.Version {
					return nil, core.Conflict
				}
				version = p.Version + 1
				topic = "suppliers.updated"
				if e := core.Changed(tx.ExecContext(ctx, `UPDATE suppliers SET name=?,canonical_name=?,rtn=?,canonical_rtn=?,contact_name=?,phone=?,email=?,address=?,country=?,canonical_country=?,website=?,notes=?,payment_terms=?,lead_time=?,evaluation=?,reliability=?,response_hours=?,is_active=?,updated_by=?,team_id=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, p.Name, CanonicalName(p.Name), p.RTN, nullable(rtn), p.ContactName, p.Phone, p.Email, p.Address, p.Country, CanonicalName(p.Country), p.Website, p.Notes, p.PaymentTerms, p.LeadTime, p.Evaluation, p.Reliability, p.ResponseHours, p.Active, a.UserID, nullable(p.TeamID), id, p.Version)); e != nil {
					return nil, dbError(e)
				}
				for _, table := range []string{"supplier_contacts", "supplier_products", "supplier_tags"} {
					if _, e := tx.ExecContext(ctx, `DELETE FROM `+table+` WHERE supplier_id=?`, id); e != nil {
						return nil, e
					}
				}
			}
			for i, c := range p.Contacts {
				if _, e := tx.ExecContext(ctx, `INSERT INTO supplier_contacts(supplier_id,name,role,phone,email,position) VALUES(?,?,?,?,?,?)`, newID, c.Name, c.Role, c.Phone, c.Email, i); e != nil {
					return nil, e
				}
			}
			for i, c := range p.Products {
				if _, e := tx.ExecContext(ctx, `INSERT INTO supplier_products(supplier_id,name,brand,part_number,description,lead_time,moq,position) VALUES(?,?,?,?,?,?,?,?)`, newID, c.Name, c.Brand, c.PartNumber, c.Description, c.LeadTime, nullable(c.MOQ), i); e != nil {
					return nil, e
				}
			}
			for kind, tags := range map[string][]string{"category": p.Categories, "brand": p.Brands} {
				for i, v := range tags {
					if _, e := tx.ExecContext(ctx, `INSERT INTO supplier_tags(supplier_id,kind,name,canonical_name,position) VALUES(?,?,?,?,?)`, newID, kind, v, CanonicalName(v), i); e != nil {
						return nil, e
					}
				}
			}
			if e := s.Core.Audit(ctx, tx, a, topic, "suppliers", newID, "success", map[string]any{"version": fmt.Sprint(version), "is_active": p.Active, "contacts": len(p.Contacts), "products": len(p.Products)}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, topic, "suppliers", newID, version); e != nil {
				return nil, e
			}
			return map[string]string{"id": newID, "version": fmt.Sprint(version)}, nil
		})
		return e
	})
	return out, dbError(e)
}
func (s *Service) Snapshot(ctx context.Context, a core.Actor, id string) (Snapshot, error) {
	c, e := s.Get(ctx, a, id)
	if e != nil {
		return Snapshot{}, e
	}
	if !c.Active {
		return Snapshot{}, core.Conflict
	}
	return Snapshot{ID: &c.ID, Name: c.Name, RTN: c.RTN, Contact: c.ContactName, Phone: c.Phone, Email: c.Email, Address: c.Address, Country: c.Country, Website: c.Website, Version: c.Version}, nil
}
func (s *Service) ResolveExact(ctx context.Context, a core.Actor, name, rtn string) (*Snapshot, error) {
	if !text(name, 180, true) {
		return nil, core.Invalid
	}
	r, e := CanonicalRTN(rtn)
	if e != nil {
		return nil, e
	}
	if e = s.gate(ctx, s.Core.DB, a, "proveedores.view"); e != nil {
		return nil, e
	}
	where, args := scope(a, "proveedores.view")
	where += ` AND c.is_active=TRUE`
	if r != "" {
		where += ` AND c.canonical_rtn=?`
		args = append(args, r)
	} else {
		if CanonicalName(name) == "" {
			return nil, core.Invalid
		}
		where += ` AND c.canonical_name=?`
		args = append(args, CanonicalName(name))
	}
	var id string
	e = s.Core.DB.QueryRowContext(ctx, `SELECT c.id FROM suppliers c WHERE `+where, args...).Scan(&id)
	if e == sql.ErrNoRows {
		return nil, nil
	}
	if e != nil {
		return nil, e
	}
	v, e := s.Snapshot(ctx, a, id)
	return &v, e
}
func (s *Service) AddNote(ctx context.Context, a core.Actor, id, key, body string, version uint64) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if !text(body, 4000, false) || version == 0 {
		return nil, core.Invalid
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
		out, e = s.Core.Idempotent(ctx, tx, a, "suppliers.note:"+id, key, map[string]any{"body": body, "version": version}, func() (any, error) {
			if c.Version != version {
				return nil, core.Conflict
			}
			r, e := tx.ExecContext(ctx, `INSERT INTO supplier_notes(supplier_id,user_id,body,created_at) VALUES(?,?,?,UTC_TIMESTAMP(6))`, id, a.UserID, body)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE suppliers SET version=version+1,updated_by=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, a.UserID, id, version)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "suppliers.noted", "suppliers", id, "success", map[string]string{"version": fmt.Sprint(version + 1), "note_id": core.WrapID(n)}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "suppliers.noted", "suppliers", id, version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": core.WrapID(n), "version": fmt.Sprint(version + 1)}, nil
		})
		return e
	})
	return out, e
}
func (s *Service) Notes(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if _, e := s.Get(ctx, a, id); e != nil {
		return nil, e
	}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM supplier_notes WHERE supplier_id=?`, id).Scan(&total); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT n.id,n.body,u.name,n.created_at FROM supplier_notes n JOIN users u ON u.id=n.user_id WHERE n.supplier_id=? ORDER BY n.id DESC LIMIT ? OFFSET ?`, id, limit, (page-1)*limit)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Note{}
	for rows.Next() {
		var n Note
		if e = rows.Scan(&n.ID, &n.Body, &n.UserName, &n.CreatedAt); e != nil {
			return nil, e
		}
		out = append(out, n)
	}
	return map[string]any{"data": out, "total": total, "page": page, "per_page": limit}, rows.Err()
}
func (s *Service) History(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if _, e := s.Get(ctx, a, id); e != nil {
		return nil, e
	}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM audit_events WHERE entity_type='suppliers' AND entity_id=? AND result='success'`, id).Scan(&total); e != nil {
		return nil, e
	}
	data, e := s.Core.Rows(ctx, `SELECT CAST(e.id AS CHAR) id,e.action,e.created_at,u.name actor_name,e.details FROM audit_events e LEFT JOIN users u ON u.id=e.requested_by WHERE e.entity_type='suppliers' AND e.entity_id=? AND e.result='success' ORDER BY e.id DESC LIMIT ? OFFSET ?`, id, limit, (page-1)*limit)
	if e != nil {
		return nil, e
	}
	return map[string]any{"data": data.(map[string]any)["data"], "total": total, "page": page, "per_page": limit}, nil
}
func (s *Service) DocumentPolicy(ctx context.Context, q core.Querier, a core.Actor, id string, write bool) error {
	permission := "proveedores.view"
	if write {
		permission = "proveedores.update"
	}
	_, e := s.access(ctx, q, a, id, permission, false, false)
	return e
}
func (s *Service) LinkDocument(ctx context.Context, a core.Actor, id, key, document string, version uint64) (any, error) {
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "proveedores.update"); e != nil {
			return e
		}
		if _, e := s.access(ctx, tx, a, id, "proveedores.update", true, true); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "suppliers.document:"+id, key, map[string]any{"document_id": document, "version": version}, func() (any, error) {
			if e := s.Core.BindDocument(ctx, tx, a, document, "suppliers", id, version); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "suppliers.document_linked", "suppliers", id, "success", map[string]string{"document_id": document}); e != nil {
				return nil, e
			}
			return map[string]string{"id": document}, nil
		})
		return e
	})
	return out, e
}
func (s *Service) Documents(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if _, e := s.Get(ctx, a, id); e != nil {
		return nil, e
	}
	predicate := `d.deleted_at IS NULL AND d.deleted_at IS NULL AND d.entity_type='suppliers' AND d.entity_id=? AND EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='core.documents.read' AND (rp.scope='all' OR (rp.scope='self' AND d.owner_id=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=d.team_id AND tm.user_id=?))))`
	args := []any{id, a.UserID, a.UserID, a.UserID}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM documents d WHERE `+predicate, args...).Scan(&total); e != nil {
		return nil, e
	}
	// Check the live service scope as well as the owner's row grants.
	if e := s.gate(ctx, s.Core.DB, a, "core.documents.read"); e != nil {
		return nil, e
	}
	data, e := s.Core.Rows(ctx, `SELECT CAST(d.id AS CHAR) id,CAST(d.version AS CHAR) version,d.current_version,v.filename,v.status FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.number=d.current_version WHERE `+predicate+` ORDER BY d.id DESC LIMIT ? OFFSET ?`, append(args, limit, (page-1)*limit)...)
	if e != nil {
		return nil, e
	}
	return map[string]any{"data": data.(map[string]any)["data"], "total": total, "page": page, "per_page": limit}, nil
}

func (s *Service) Teams(ctx context.Context, a core.Actor) (any, error) {
	if e := s.gate(ctx, s.Core.DB, a, "proveedores.view"); e != nil {
		return nil, e
	}
	return s.Core.Rows(ctx, `SELECT CAST(t.id AS CHAR) id,t.name FROM teams t WHERE EXISTS(SELECT 1 FROM team_members tm WHERE tm.team_id=t.id AND tm.user_id=?) OR EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission IN ('proveedores.create','proveedores.update') AND rp.scope='all') ORDER BY t.name,t.id`, a.UserID, a.UserID)
}

// LegacySnapshot preserves text-only historical references without master creation.
func LegacySnapshot(name string) (Snapshot, error) {
	if !text(name, 180, false) {
		return Snapshot{}, core.Invalid
	}
	return Snapshot{Name: name}, nil
}
