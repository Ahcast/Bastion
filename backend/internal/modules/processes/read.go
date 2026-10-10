package processes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"strings"
	"time"
)

type Process struct {
	ID              string     `json:"id"`
	Entity          string     `json:"entity"`
	Reference       string     `json:"reference"`
	Description     string     `json:"description"`
	Deadline        string     `json:"deadline"`
	Active          bool       `json:"is_source_active"`
	Current         *bool      `json:"is_current"`
	Version         uint64     `json:"version,string"`
	Owner           string     `json:"created_by"`
	Team            string     `json:"team_id"`
	Interesting     bool       `json:"is_interesting"`
	Note            string     `json:"note"`
	PersonalVersion uint64     `json:"personal_version,string"`
	Documents       []Document `json:"documents"`
}
type Filter struct {
	export             bool
	Search             string `json:"search"`
	Entity             string `json:"entity"`
	Interesting        *bool  `json:"interesting,omitempty"`
	Active             *bool  `json:"source_active,omitempty"`
	Current            *bool  `json:"current,omitempty"`
	HideMunicipalities bool   `json:"hide_municipalities"`
	Direction          string `json:"direction"`
	DateFrom           string `json:"date_from"`
	DateTo             string `json:"date_to"`
	Page               int    `json:"page"`
	PerPage            int    `json:"per_page"`
}
type List struct {
	Data    []Process `json:"data"`
	Total   int64     `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, p string) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, p, a.UserID, team, false)
}

const columns = `c.id,c.entity,c.reference,c.description,c.deadline,c.is_source_active,c.closing_at,c.version,c.created_by,COALESCE(CAST(c.team_id AS CHAR),''),COALESCE(m.is_interesting,FALSE),COALESCE(m.note,''),COALESCE(m.version,0),c.normalized_data`

func scan(row interface{ Scan(...any) error }) (Process, error) {
	var p Process
	var closing sql.NullTime
	var raw string
	e := row.Scan(&p.ID, &p.Entity, &p.Reference, &p.Description, &p.Deadline, &p.Active, &closing, &p.Version, &p.Owner, &p.Team, &p.Interesting, &p.Note, &p.PersonalVersion, &raw)
	if e != nil {
		return p, e
	}
	if closing.Valid {
		v := !closing.Time.Before(time.Now().UTC())
		p.Current = &v
	}
	var state State
	if json.Unmarshal([]byte(raw), &state) != nil {
		return p, core.Conflict
	}
	p.Documents = state.Documents
	return p, nil
}
func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, p string) (Process, error) {
	if core.ID(id) != nil {
		return Process{}, core.Invalid
	}
	x, e := scan(q.QueryRowContext(ctx, `SELECT `+columns+` FROM processes c LEFT JOIN process_user_marks m ON m.process_id=c.id AND m.user_id=? WHERE c.id=?`, a.UserID, id))
	if e == sql.ErrNoRows {
		return x, core.Missing
	}
	if e != nil {
		return x, e
	}
	if e = s.Core.Authorize(ctx, q, a, p, x.Owner, x.Team, false); e == core.Forbidden {
		return x, core.Missing
	}
	return x, e
}
func (s *Service) Get(ctx context.Context, a core.Actor, id string) (Process, error) {
	return s.access(ctx, s.Core.DB, a, id, "procesos.view")
}
func literal(v string) string {
	return "%" + strings.NewReplacer("!", "!!", "%", "!%", "_", "!_").Replace(v) + "%"
}
func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (List, error) {
	out := List{Data: []Process{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || !text(f.Search, 500) || !text(f.Entity, 1000) || (f.Direction != "" && f.Direction != "asc" && f.Direction != "desc") {
		return out, core.Invalid
	}
	if e := s.gate(ctx, s.Core.DB, a, "procesos.view"); e != nil {
		return out, e
	}
	where := `EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=? AND rp.permission='procesos.view' AND (rp.scope='all' OR (rp.scope='self' AND c.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.user_id=? AND tm.team_id=c.team_id))))`
	args := []any{a.UserID, a.UserID, a.UserID}
	if f.export {
		if e := s.gate(ctx, s.Core.DB, a, "procesos.export"); e != nil {
			return out, e
		}
		where += " AND " + strings.ReplaceAll(where, "procesos.view", "procesos.export")
		args = append(args, a.UserID, a.UserID, a.UserID)
	}
	if f.Search != "" {
		where += ` AND (c.reference LIKE ? ESCAPE '!' OR c.description LIKE ? ESCAPE '!' OR c.entity LIKE ? ESCAPE '!')`
		args = append(args, literal(f.Search), literal(f.Search), literal(f.Search))
	}
	if f.Entity != "" {
		where += ` AND c.entity=?`
		args = append(args, f.Entity)
	}
	if f.Interesting != nil {
		where += ` AND COALESCE(m.is_interesting,FALSE)=?`
		args = append(args, *f.Interesting)
	}
	if f.Active != nil {
		where += ` AND c.is_source_active=?`
		args = append(args, *f.Active)
	}
	if f.Current != nil {
		if *f.Current {
			where += ` AND c.closing_at>=UTC_TIMESTAMP(6)`
		} else {
			where += ` AND c.closing_at<UTC_TIMESTAMP(6)`
		}
	}
	if f.HideMunicipalities {
		where += ` AND LOWER(c.entity) NOT LIKE '%municipal%'`
	}
	for _, bound := range []struct{ value, operator string }{{f.DateFrom, ">="}, {f.DateTo, "<"}} {
		if bound.value == "" {
			continue
		}
		t, e := time.Parse("2006-01-02", bound.value)
		if e != nil {
			return out, core.Invalid
		}
		t = t.Add(6 * time.Hour)
		if bound.operator == "<" {
			t = t.AddDate(0, 0, 1)
		}
		where += " AND c.closing_at" + bound.operator + "?"
		args = append(args, t)
	}
	join := ` FROM processes c LEFT JOIN process_user_marks m ON m.process_id=c.id AND m.user_id=? WHERE `
	queryArgs := append([]any{a.UserID}, args...)
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*)`+join+where, queryArgs...).Scan(&out.Total); e != nil {
		return out, e
	}
	direction := "DESC"
	if f.Direction == "asc" {
		direction = "ASC"
	}
	queryArgs = append(queryArgs, f.PerPage, (f.Page-1)*f.PerPage)
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT `+columns+join+where+` ORDER BY c.closing_at `+direction+`,c.id `+direction+` LIMIT ? OFFSET ?`, queryArgs...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		p, e := scan(rows)
		if e != nil {
			return out, e
		}
		out.Data = append(out.Data, p)
	}
	return out, rows.Err()
}

type PersonalInput struct {
	Interesting *bool   `json:"is_interesting,omitempty"`
	Note        *string `json:"note,omitempty"`
	Version     uint64  `json:"version,string"`
}

func (s *Service) Personal(ctx context.Context, a core.Actor, id, k, kind string, p PersonalInput) (any, error) {
	if a.Type != "user" || (kind != "mark" && kind != "note") || (kind == "mark" && (p.Interesting == nil || p.Note != nil)) || (kind == "note" && (p.Note == nil || p.Interesting != nil || !text(*p.Note, 4000))) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if _, e := s.access(ctx, tx, a, id, "procesos.view"); e != nil {
			return e
		}
		if _, e := s.access(ctx, tx, a, id, "procesos."+kind); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "processes."+kind+":"+id, k, p, func() (any, error) {
			_, e := tx.ExecContext(ctx, `INSERT INTO process_user_marks VALUES(?,?,FALSE,'',0,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE user_id=VALUES(user_id)`, id, a.UserID)
			if e != nil {
				return nil, e
			}
			var version uint64
			if e = tx.QueryRowContext(ctx, `SELECT version FROM process_user_marks WHERE process_id=? AND user_id=? FOR UPDATE`, id, a.UserID).Scan(&version); e != nil {
				return nil, e
			}
			if version != p.Version {
				return nil, core.Conflict
			}
			field := "note"
			var v any = p.Note
			if kind == "mark" {
				field = "is_interesting"
				v = *p.Interesting
			} else {
				v = *p.Note
			}
			_, e = tx.ExecContext(ctx, `UPDATE process_user_marks SET `+field+`=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE process_id=? AND user_id=?`, v, id, a.UserID)
			if e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "processes."+kind, "processes", id, "success", map[string]string{}); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "personal_version": fmtVersion(version + 1)}, nil
		})
		return e
	})
	return out, e
}
func fmtVersion(v uint64) string { return fmt.Sprintf("%d", v) }
func (s *Service) Versions(ctx context.Context, a core.Actor, id string, page, limit int) (any, error) {
	if page < 1 || page > 100000 || limit < 1 || limit > 100 {
		return nil, core.Invalid
	}
	if _, e := s.Get(ctx, a, id); e != nil {
		return nil, e
	}
	var total int
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM process_versions WHERE process_id=?`, id).Scan(&total); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT id,CAST(version AS CHAR),content_hash,normalized_data,changes,created_at FROM process_versions WHERE process_id=? ORDER BY version DESC LIMIT ? OFFSET ?`, id, limit, (page-1)*limit)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []map[string]any{}
	for rows.Next() {
		var vid, v, h, data, changes string
		var at time.Time
		if e = rows.Scan(&vid, &v, &h, &data, &changes, &at); e != nil {
			return nil, e
		}
		out = append(out, map[string]any{"id": vid, "version": v, "content_hash": h, "data": json.RawMessage(data), "changes": json.RawMessage(changes), "created_at": at})
	}
	if e = rows.Err(); e != nil {
		return nil, e
	}
	rows.Close()
	for _, v := range out {
		d, e := changes(ctx, s.Core.DB, id, v["version"].(string))
		if e != nil {
			return nil, e
		}
		v["diff"] = d
	}
	return map[string]any{"data": out, "total": total, "page": page, "per_page": limit}, rows.Err()
}
