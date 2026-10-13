package ai

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"time"
)

type runState struct {
	Run
	Actor, Hash, Cipher string
}

func run(ctx context.Context, tx *sql.Tx, task, id string) (runState, error) {
	var r runState
	e := tx.QueryRowContext(ctx, `SELECT CAST(id AS CHAR),attempt,status,skill_version,lease_expires_at,renewals,started_at,finished_at,error_code,actor_key,COALESCE(lease_hash,''),COALESCE(lease_cipher,'') FROM ai_runs WHERE id=? AND task_id=? FOR UPDATE`, id, task).Scan(&r.ID, &r.Attempt, &r.Status, &r.SkillVersion, &r.Expires, &r.Renewals, &r.Started, &r.Finished, &r.Code, &r.Actor, &r.Hash, &r.Cipher)
	if e == sql.ErrNoRows {
		e = LeaseExpired
	}
	return r, e
}
func dbNow(ctx context.Context, tx *sql.Tx) (time.Time, error) {
	var now time.Time
	e := tx.QueryRowContext(ctx, `SELECT UTC_TIMESTAMP(6)`).Scan(&now)
	return now, e
}
func (s *Service) requested(ctx context.Context, q core.Querier, t Task, src SourceInput) error {
	for _, p := range []string{"procesos.view", "ai.tasks.read", "ai.tasks.create"} {
		if e := s.Core.AuthorizeGrantHolder(ctx, q, t.RequestedBy, p, src.Owner, src.Team); e != nil {
			return e
		}
	}
	return nil
}

type claimRef struct {
	TaskID string `json:"task_id"`
	RunID  string `json:"run_id"`
}
type claimRefs struct {
	Data []claimRef `json:"data"`
}

func (s *Service) Claim(ctx context.Context, a core.Actor, key string, p ClaimInput) (Claims, error) {
	out := Claims{Data: []Reservation{}}
	if p.Schema != "1.0" || p.Limit < 1 || p.Limit > 10 {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		for _, perm := range []string{"ai.tasks.read", "ai.tasks.execute"} {
			if e := s.gate(ctx, tx, a, perm); e != nil {
				return e
			}
		}
		refs, e := s.Core.Idempotent(ctx, tx, a, "ai.claim", key, p, func() (any, error) {
			read, args := s.scope(a, "ai.tasks.read")
			exec, x := s.Processes.ScopeSQL(a, "ai.tasks.execute")
			args = append(args, x...)
			// A bounded scan, numeric source order shared across competing actors. Lock
			// after taking the owner lock and then recheck; MySQL 5.7 has no SKIP LOCKED.
			rows, e := tx.QueryContext(ctx, `SELECT CAST(t.id AS CHAR) FROM ai_tasks t WHERE `+read+` AND (`+exec+`) AND (`+s.Processes.EligibleSQL()+`) AND ((t.status='pending' AND t.available_at<=UTC_TIMESTAMP(6)) OR (t.status='running' AND EXISTS(SELECT 1 FROM ai_runs r WHERE r.id=t.current_run_id AND r.lease_expires_at<=UTC_TIMESTAMP(6)))) ORDER BY t.process_id,t.id LIMIT 100`, args...)
			if e != nil {
				return nil, e
			}
			ids := []string{}
			for rows.Next() {
				var id string
				if e = rows.Scan(&id); e != nil {
					rows.Close()
					return nil, e
				}
				ids = append(ids, id)
			}
			e = rows.Err()
			rows.Close()
			if e != nil {
				return nil, e
			}
			found := claimRefs{Data: []claimRef{}}
			for _, id := range ids {
				if len(found.Data) >= p.Limit {
					break
				}
				t, src, e := s.access(ctx, tx, a, id, "ai.tasks.execute")
				if e == core.Missing {
					continue
				}
				if e != nil {
					return nil, e
				}
				if !current(t, src) {
					continue
				}
				if e = s.requested(ctx, tx, t, src); e == core.Forbidden || e == core.Unauthorized {
					continue
				} else if e != nil {
					return nil, e
				}
				now, e := dbNow(ctx, tx)
				if e != nil {
					return nil, e
				}
				if t.Status == "running" {
					r, e := run(ctx, tx, t.ID, t.CurrentRun)
					if e != nil {
						return nil, e
					}
					if r.Expires.After(now) {
						continue
					}
					if _, e = tx.ExecContext(ctx, `UPDATE ai_runs SET status='expired',lease_hash=NULL,lease_cipher=NULL,finished_at=? WHERE id=?`, now, r.ID); e != nil {
						return nil, e
					}
					t.CurrentRun = ""
					t.Version++
					if t.Attempts >= 3 {
						t.Status = "needs_review"
					} else {
						t.Status = "pending"
					}
					if _, e = tx.ExecContext(ctx, `UPDATE ai_tasks SET status=?,version=?,current_run_id=NULL,available_at=?,updated_at=? WHERE id=?`, t.Status, t.Version, now, now, t.ID); e != nil {
						return nil, e
					}
					if e = s.event(ctx, tx, a, t, "ai.lease_expired"); e != nil {
						return nil, e
					}
				}
				if t.Status != "pending" || t.Available.After(now) || t.Attempts >= 3 {
					continue
				}
				token := security.Token()
				cipher, e := security.Encrypt(s.Core.Config.Key, token)
				if e != nil {
					return nil, e
				}
				t.Attempts++
				t.Version++
				t.Status = "running"
				rr, e := tx.ExecContext(ctx, `INSERT INTO ai_runs(task_id,attempt,actor_key,actor_user_id,skill_version,status,lease_hash,lease_cipher,lease_expires_at,started_at) VALUES(?,?,?,?,?,'running',?,?,?,?)`, t.ID, t.Attempts, a.Key(), a.UserID, t.SkillVersion, security.Hash(token), cipher, now.Add(5*time.Minute), now)
				if e != nil {
					return nil, e
				}
				rid, e := rr.LastInsertId()
				if e != nil {
					return nil, e
				}
				t.CurrentRun = core.WrapID(rid)
				if e = core.Changed(tx.ExecContext(ctx, `UPDATE ai_tasks SET status='running',attempts=?,version=?,current_run_id=?,updated_at=? WHERE id=?`, t.Attempts, t.Version, t.CurrentRun, now, t.ID)); e != nil {
					return nil, e
				}
				if e = s.event(ctx, tx, a, t, "ai.run_started"); e != nil {
					return nil, e
				}
				found.Data = append(found.Data, claimRef{t.ID, t.CurrentRun})
			}
			return found, nil
		})
		if e != nil {
			return e
		}
		var saved claimRefs
		if e = convert(refs, &saved); e != nil {
			return e
		}
		for _, ref := range saved.Data {
			t, src, e := s.access(ctx, tx, a, ref.TaskID, "ai.tasks.execute")
			if e != nil {
				return e
			}
			if e = s.requested(ctx, tx, t, src); e != nil {
				return e
			}
			if !current(t, src) {
				return InputStale
			}
			rr, e := run(ctx, tx, t.ID, ref.RunID)
			if e != nil {
				return e
			}
			now, e := dbNow(ctx, tx)
			if e != nil {
				return e
			}
			if t.Status != "running" || t.CurrentRun != rr.ID || rr.Actor != a.Key() || rr.Status != "running" || !rr.Expires.After(now) {
				return LeaseExpired
			}
			token, e := security.Decrypt(s.Core.Config.Key, rr.Cipher)
			if e != nil {
				return e
			}
			var snapshot []byte
			if e = tx.QueryRowContext(ctx, `SELECT input_snapshot FROM ai_tasks WHERE id=?`, t.ID).Scan(&snapshot); e != nil {
				return e
			}
			out.Data = append(out.Data, Reservation{t, rr.ID, token, rr.Expires, snapshot, "UNTRUSTED_EXTERNAL_CONTENT"})
		}
		return nil
	})
	return out, e
}
func verify(t Task, src SourceInput, a core.Actor, p ExecutionInput, r runState, now time.Time, active bool) error {
	if !current(t, src) || p.InputVersion != t.InputVersion || p.InputHash != t.InputHash {
		return InputStale
	}
	if r.Actor != a.Key() || p.SkillVersion != t.SkillVersion || p.SkillVersion != r.SkillVersion || !security.Equal(r.Hash, security.Hash(p.Lease)) {
		return LeaseExpired
	}
	if !r.Expires.After(now) || (active && (t.Status != "running" || t.CurrentRun != r.ID || r.Status != "running")) {
		return LeaseExpired
	}
	return nil
}
func (s *Service) execution(ctx context.Context, a core.Actor, id, key, operation string, p ExecutionInput, body any, fn func(*sql.Tx, *Task, runState, time.Time) error) (Task, error) {
	var out Task
	if !validExecution(p) {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		t, src, e := s.access(ctx, tx, a, id, "ai.tasks.execute")
		if e != nil {
			return e
		}
		if e = s.requested(ctx, tx, t, src); e != nil {
			return e
		}
		r, e := run(ctx, tx, id, p.RunID)
		if e != nil {
			return e
		}
		now, e := dbNow(ctx, tx)
		if e != nil {
			return e
		}
		// Validate lease/source even before an idempotent replay. Failed run replays
		// remain available until the original lease deadline, unless replaced/cancelled.
		if e = verify(t, src, a, p, r, now, false); e != nil {
			return e
		}
		if r.Attempt != t.Attempts || t.Status == "cancelled" || (t.CurrentRun != "" && t.CurrentRun != r.ID) {
			return LeaseExpired
		}
		v, e := s.Core.Idempotent(ctx, tx, a, "ai."+operation+":"+id, key, body, func() (any, error) {
			if e := verify(t, src, a, p, r, now, true); e != nil {
				return nil, e
			}
			if t.Version != p.Version {
				return nil, core.Conflict
			}
			if e := fn(tx, &t, r, now); e != nil {
				return nil, e
			}
			if e := s.event(ctx, tx, a, t, "ai.run_"+operation); e != nil {
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
func (s *Service) Renew(ctx context.Context, a core.Actor, id, key string, p ExecutionInput) (Task, error) {
	return s.execution(ctx, a, id, key, "renewed", p, p, func(tx *sql.Tx, t *Task, r runState, now time.Time) error {
		until := now.Add(5 * time.Minute)
		cap := r.Started.Add(15 * time.Minute)
		if until.After(cap) {
			until = cap
		}
		if r.Renewals >= 2 || !until.After(r.Expires) {
			return core.Conflict
		}
		if e := core.Changed(tx.ExecContext(ctx, `UPDATE ai_runs SET lease_expires_at=?,renewals=renewals+1 WHERE id=? AND lease_expires_at>UTC_TIMESTAMP(6)`, until, r.ID)); e != nil {
			return e
		}
		t.Version++
		t.Updated = now
		return core.Changed(tx.ExecContext(ctx, `UPDATE ai_tasks SET version=?,updated_at=? WHERE id=?`, t.Version, now, t.ID))
	})
}
func (s *Service) Fail(ctx context.Context, a core.Actor, id, key string, p FailInput) (Task, error) {
	if p.Code != "transient" && p.Code != "executor_limit" && p.Code != "invalid_input" && p.Code != "permission_denied" {
		return Task{}, core.Invalid
	}
	return s.execution(ctx, a, id, key, "failed", p.ExecutionInput, p, func(tx *sql.Tx, t *Task, r runState, now time.Time) error {
		t.Status = "needs_review"
		if p.Code == "transient" || p.Code == "executor_limit" {
			t.Status = "failed"
			if t.Attempts < 3 {
				t.Status = "pending"
				t.Available = now.Add(time.Duration(1<<uint(t.Attempts-1)) * time.Minute)
			}
		}
		if e := core.Changed(tx.ExecContext(ctx, `UPDATE ai_runs SET status='failed',lease_cipher=NULL,finished_at=?,error_code=? WHERE id=? AND lease_expires_at>UTC_TIMESTAMP(6)`, now, p.Code, r.ID)); e != nil {
			return e
		}
		t.Version++
		t.CurrentRun = ""
		t.Updated = now
		return core.Changed(tx.ExecContext(ctx, `UPDATE ai_tasks SET status=?,version=?,current_run_id=NULL,available_at=?,updated_at=? WHERE id=?`, t.Status, t.Version, t.Available, now, t.ID))
	})
}
func (s *Service) Cancel(ctx context.Context, a core.Actor, id, key string, p CancelInput) (Task, error) {
	var out Task
	if a.Type != "user" {
		return out, core.Forbidden
	}
	if p.Schema != "1.0" || p.Version == 0 {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		t, _, e := s.access(ctx, tx, a, id, "ai.tasks.cancel")
		if e != nil {
			return e
		}
		v, e := s.Core.Idempotent(ctx, tx, a, "ai.cancel:"+id, key, p, func() (any, error) {
			if t.Version != p.Version || t.Status == "cancelled" {
				return nil, core.Conflict
			}
			if t.CurrentRun != "" {
				if _, e = tx.ExecContext(ctx, `UPDATE ai_runs SET status='cancelled',lease_hash=NULL,lease_cipher=NULL,finished_at=UTC_TIMESTAMP(6) WHERE id=? AND task_id=?`, t.CurrentRun, t.ID); e != nil {
					return nil, e
				}
			}
			t.Status = "cancelled"
			now, e := dbNow(ctx, tx)
			if e != nil {
				return nil, e
			}
			t.Updated = now
			t.CurrentRun = ""
			t.Version++
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE ai_tasks SET status='cancelled',version=?,current_run_id=NULL,updated_at=UTC_TIMESTAMP(6) WHERE id=?`, t.Version, t.ID)); e != nil {
				return nil, e
			}
			if e = s.event(ctx, tx, a, t, "ai.task_cancelled"); e != nil {
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
