package ai

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
)

const taskColumns = `CAST(t.id AS CHAR),t.task_type,CAST(t.process_id AS CHAR),t.input_version,t.input_hash,t.skill_version,CAST(t.requested_by AS CHAR),t.status,t.version,t.attempts,t.available_at,COALESCE(CAST(t.current_run_id AS CHAR),''),t.created_at,t.updated_at`

func scanTask(row interface{ Scan(...any) error }) (Task, error) {
	var t Task
	e := row.Scan(&t.ID, &t.Type, &t.ProcessID, &t.InputVersion, &t.InputHash, &t.SkillVersion, &t.RequestedBy, &t.Status, &t.Version, &t.Attempts, &t.Available, &t.CurrentRun, &t.Created, &t.Updated)
	if e == sql.ErrNoRows {
		e = core.Missing
	}
	return t, e
}
func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, permission string) error {
	if s.Processes == nil {
		return core.Disabled
	}
	var team string
	e := q.QueryRowContext(ctx, `SELECT CAST(team_id AS CHAR) FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, permission, a.UserID, team, false)
}

// Always lock authenticated user before owner rows, consistent with Core writes.
func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, permission string) (Task, SourceInput, error) {
	var src SourceInput
	if !canonical(id) {
		return Task{}, src, core.Invalid
	}
	if e := s.gate(ctx, q, a, "procesos.view"); e != nil {
		return Task{}, src, e
	}
	if e := s.gate(ctx, q, a, "ai.tasks.read"); e != nil {
		return Task{}, src, e
	}
	if permission != "ai.tasks.read" {
		if e := s.gate(ctx, q, a, permission); e != nil {
			return Task{}, src, e
		}
	}
	t, e := scanTask(q.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM ai_tasks t WHERE t.id=?`, id))
	if e != nil {
		return t, src, e
	}
	src, e = s.Processes.Snapshot(ctx, q, a, t.ProcessID)
	if e != nil {
		return t, src, e
	}
	for _, p := range []string{"ai.tasks.read", permission} {
		if e = s.Core.Authorize(ctx, q, a, p, src.Owner, src.Team, false); e != nil {
			if e == core.Forbidden {
				e = core.Missing
			}
			return t, src, e
		}
	}
	if _, ok := q.(*sql.Tx); ok {
		t, e = scanTask(q.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM ai_tasks t WHERE t.id=? FOR UPDATE`, id))
	}
	return t, src, e
}
func current(t Task, p SourceInput) bool { return t.InputVersion == p.Version && t.InputHash == p.Hash }
func (s *Service) event(ctx context.Context, tx *sql.Tx, a core.Actor, t Task, action string) error {
	if e := s.Core.Audit(ctx, tx, a, action, "ai_task", t.ID, "success", map[string]any{"status": t.Status, "version": t.Version, "attempts": t.Attempts}); e != nil {
		return e
	}
	return s.Core.EmitEvent(ctx, tx, a, action, "ai_task", t.ID, t.Version)
}
func convert(v any, out any) error {
	b, e := json.Marshal(v)
	if e != nil {
		return e
	}
	return json.Unmarshal(b, out)
}
func (s *Service) Create(ctx context.Context, a core.Actor, key string, p CreateInput) (Task, error) {
	var out Task
	if a.Type != "user" {
		return out, core.Forbidden
	}
	if p.Schema != "1.0" || !canonical(p.ProcessID) || p.InputVersion == 0 || !skill.MatchString(p.SkillVersion) {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "ai.tasks.read"); e != nil {
			return e
		}
		if e := s.gate(ctx, tx, a, "ai.tasks.create"); e != nil {
			return e
		}
		src, e := s.Processes.Snapshot(ctx, tx, a, p.ProcessID)
		if e != nil {
			return e
		}
		for _, perm := range []string{"ai.tasks.read", "ai.tasks.create"} {
			if e = s.Core.Authorize(ctx, tx, a, perm, src.Owner, src.Team, false); e != nil {
				return e
			}
		}
		if p.InputVersion != src.Version {
			return InputStale
		}
		if !hash.MatchString(src.Hash) || len(src.Data) > 256*1024 || !json.Valid(src.Data) {
			return core.Invalid
		}
		v, e := s.Core.Idempotent(ctx, tx, a, "ai.create:"+p.ProcessID, key, p, func() (any, error) {
			t, e := scanTask(tx.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM ai_tasks t WHERE task_type='process_score' AND process_id=? FOR UPDATE`, p.ProcessID))
			if e == nil {
				if t.InputVersion != src.Version || t.InputHash != src.Hash || t.SkillVersion != p.SkillVersion {
					return nil, core.Conflict
				}
				return t, nil
			}
			if e != core.Missing {
				return nil, e
			}
			r, e := tx.ExecContext(ctx, `INSERT INTO ai_tasks(task_type,process_id,input_version,input_hash,input_snapshot,skill_version,requested_by,status,available_at,created_at,updated_at) VALUES('process_score',?,?,?,?,?,?,'pending',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, p.ProcessID, src.Version, src.Hash, string(src.Data), p.SkillVersion, a.UserID)
			if e != nil {
				return nil, e
			}
			id, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			t, e = scanTask(tx.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM ai_tasks t WHERE id=?`, id))
			if e != nil {
				return nil, e
			}
			if e = s.event(ctx, tx, a, t, "ai.task_created"); e != nil {
				return nil, e
			}
			return t, nil
		})
		if e != nil {
			return e
		}
		return convert(v, &out)
	})
	return out, e
}
func (s *Service) Get(ctx context.Context, a core.Actor, id string) (Detail, error) {
	out := Detail{Runs: []Run{}}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		t, _, e := s.access(ctx, tx, a, id, "ai.tasks.read")
		if e != nil {
			return e
		}
		out.Task = t
		rows, e := tx.QueryContext(ctx, `SELECT CAST(id AS CHAR),attempt,status,skill_version,lease_expires_at,renewals,started_at,finished_at,error_code FROM ai_runs WHERE task_id=? ORDER BY attempt DESC LIMIT 3`, id)
		if e != nil {
			return e
		}
		defer rows.Close()
		for rows.Next() {
			var r Run
			if e = rows.Scan(&r.ID, &r.Attempt, &r.Status, &r.SkillVersion, &r.Expires, &r.Renewals, &r.Started, &r.Finished, &r.Code); e != nil {
				return e
			}
			out.Runs = append(out.Runs, r)
		}
		return rows.Err()
	})
	return out, e
}
func (s *Service) scope(a core.Actor, permission string) (string, []any) {
	read, args := s.Processes.ScopeSQL(a, "procesos.view")
	task, params := s.Processes.ScopeSQL(a, permission)
	return "(" + read + ") AND (" + task + ")", append(args, params...)
}
func (s *Service) List(ctx context.Context, a core.Actor, f Filter) (Listing, error) {
	out := Listing{Data: []Task{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || (f.ProcessID != "" && !canonical(f.ProcessID)) || (f.Status != "" && !validStatus(f.Status)) {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "ai.tasks.read"); e != nil {
			return e
		}
		if e := s.gate(ctx, tx, a, "procesos.view"); e != nil {
			return e
		}
		where, args := s.scope(a, "ai.tasks.read")
		if f.ProcessID != "" {
			where += " AND t.process_id=?"
			args = append(args, f.ProcessID)
		}
		if f.Status != "" {
			where += " AND t.status=?"
			args = append(args, f.Status)
		}
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM ai_tasks t WHERE `+where, args...).Scan(&out.Total); e != nil {
			return e
		}
		args = append(args, f.PerPage, (f.Page-1)*f.PerPage)
		rows, e := tx.QueryContext(ctx, `SELECT `+taskColumns+` FROM ai_tasks t WHERE `+where+` ORDER BY t.id DESC LIMIT ? OFFSET ?`, args...)
		if e != nil {
			return e
		}
		defer rows.Close()
		for rows.Next() {
			t, e := scanTask(rows)
			if e != nil {
				return e
			}
			out.Data = append(out.Data, t)
		}
		return rows.Err()
	})
	return out, e
}
