package collaboration

import (
	"context"
	"database/sql"
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type Task struct {
	ID            string    `json:"id"`
	CreatorID     string    `json:"creator_id"`
	AssigneeID    string    `json:"assignee_id"`
	AssigneeName  string    `json:"assignee_name"`
	ResourceType  string    `json:"resource_type,omitempty"`
	ResourceID    string    `json:"resource_id,omitempty"`
	ResourceLabel string    `json:"resource_label,omitempty"`
	Title         string    `json:"title"`
	Note          string    `json:"note,omitempty"`
	DueDate       string    `json:"due_date,omitempty"`
	Status        string    `json:"status"`
	Version       uint64    `json:"version,string"`
	CreatedAt     time.Time `json:"created_at"`
	UpdatedAt     time.Time `json:"updated_at"`
}

type TaskPage struct {
	Data    []Task `json:"data"`
	Total   int    `json:"total"`
	Page    int    `json:"page"`
	PerPage int    `json:"per_page"`
}

type TaskInput struct {
	AssigneeID   string `json:"assignee_id"`
	ResourceType string `json:"resource_type"`
	ResourceID   string `json:"resource_id"`
	Title        string `json:"title"`
	Note         string `json:"note"`
	DueDate      string `json:"due_date"`
}

type TaskUpdate struct {
	Version    uint64 `json:"version,string"`
	AssigneeID string `json:"assignee_id"`
	Title      string `json:"title"`
	Note       string `json:"note"`
	DueDate    string `json:"due_date"`
}

type TaskStatusInput struct {
	Version uint64 `json:"version,string"`
	Status  string `json:"status"`
}

type TaskReopenInput struct {
	Version uint64 `json:"version,string"`
}

type TaskFilter struct {
	View    string
	Status  string
	Page    int
	PerPage int
}

type Assignee struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

func validTaskStatus(v string) bool {
	return v == "open" || v == "in_progress" || v == "done" || v == "cancelled"
}

func activeTaskStatus(v string) bool { return v == "open" || v == "in_progress" }

func validDueDate(v string) bool {
	if v == "" {
		return true
	}
	t, e := time.Parse("2006-01-02", v)
	return e == nil && t.Format("2006-01-02") == v && t.Year() >= 1000
}

func taskToday() string {
	return time.Now().In(time.FixedZone("America/Tegucigalpa", -6*60*60)).Format("2006-01-02")
}

func validateTaskInput(p TaskInput) error {
	p.Title = strings.TrimSpace(p.Title)
	p.Note = strings.TrimSpace(p.Note)
	if core.ID(p.AssigneeID) != nil || !validName(p.Title, 180) || len([]rune(p.Note)) > 4000 || strings.ContainsRune(p.Note, '\x00') || !validDueDate(p.DueDate) {
		return core.Invalid
	}
	if (p.ResourceType == "") != (p.ResourceID == "") || p.ResourceType != "" && core.ID(p.ResourceID) != nil {
		return core.Invalid
	}
	return nil
}

// Task writes can require FK locks on both participants. Acquire user rows in
// numeric order before authorization/task locks, then recheck the participant
// snapshot under the task lock. This also protects concurrent reassignment.
func lockTaskUsers(ctx context.Context, tx *sql.Tx, ids ...string) error {
	values := make([]uint64, 0, len(ids))
	seen := map[uint64]bool{}
	for _, id := range ids {
		n, e := strconv.ParseUint(id, 10, 64)
		if e != nil || n == 0 {
			return core.Invalid
		}
		if !seen[n] {
			seen[n] = true
			values = append(values, n)
		}
	}
	sort.Slice(values, func(i, j int) bool { return values[i] < values[j] })
	for _, id := range values {
		var found uint64
		if e := tx.QueryRowContext(ctx, `SELECT id FROM users WHERE id=? FOR UPDATE`, id).Scan(&found); e == sql.ErrNoRows {
			return core.Missing
		} else if e != nil {
			return e
		}
	}
	return nil
}
func (s *Service) lockTaskParticipants(ctx context.Context, tx *sql.Tx, a core.Actor, id string, extra ...string) error {
	var creator, assignee string
	if e := tx.QueryRowContext(ctx, `SELECT CAST(creator_id AS CHAR),CAST(assignee_id AS CHAR) FROM collaboration_tasks WHERE id=?`, id).Scan(&creator, &assignee); e == sql.ErrNoRows {
		return core.Missing
	} else if e != nil {
		return e
	}
	if a.UserID != creator && a.UserID != assignee {
		return core.Missing
	}
	ids := append([]string{creator, assignee, a.UserID}, extra...)
	if e := lockTaskUsers(ctx, tx, ids...); e != nil {
		return e
	}
	var currentCreator, currentAssignee string
	if e := tx.QueryRowContext(ctx, `SELECT CAST(creator_id AS CHAR),CAST(assignee_id AS CHAR) FROM collaboration_tasks WHERE id=? FOR UPDATE`, id).Scan(&currentCreator, &currentAssignee); e != nil {
		return e
	}
	if creator != currentCreator || assignee != currentAssignee {
		return core.Conflict
	}
	return nil
}

func (s *Service) activeUser(ctx context.Context, q core.Querier, id string) error {
	var active bool
	var secret sql.NullString
	if e := q.QueryRowContext(ctx, `SELECT active,totp_secret FROM users WHERE id=?`, id).Scan(&active, &secret); e == sql.ErrNoRows {
		return core.Invalid
	} else if e != nil {
		return e
	}
	if !active || !secret.Valid {
		return core.Invalid
	}
	return nil
}

func (s *Service) validateTaskAssignee(ctx context.Context, q core.Querier, id string, r *Resource) error {
	if e := s.activeUser(ctx, q, id); e != nil {
		return e
	}
	if r != nil {
		if e := s.grantHolderCanResource(ctx, q, id, *r); e != nil {
			if e == core.Forbidden {
				return core.Invalid
			}
			return e
		}
		return nil
	}
	if e := s.Core.AuthorizeGrantHolder(ctx, q, id, "colaboracion.view", id, ""); e != nil {
		if e == core.Forbidden {
			return core.Invalid
		}
		return e
	}
	return nil
}

func (s *Service) CreateTask(ctx context.Context, a core.Actor, key string, p TaskInput) (any, error) {
	p.Title, p.Note = strings.TrimSpace(p.Title), strings.TrimSpace(p.Note)
	if a.Type != "user" || key == "" || len(key) > 100 || validateTaskInput(p) != nil {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := lockTaskUsers(ctx, tx, a.UserID, p.AssigneeID); e != nil {
			return e
		}
		if e := s.authorizeAny(ctx, tx, a, "colaboracion.view"); e != nil {
			return e
		}
		var resource *Resource
		if p.ResourceType != "" {
			r, e := s.resource(ctx, tx, a, p.ResourceType, p.ResourceID)
			if e != nil {
				return e
			}
			if e = resourcePermission(ctx, tx, s, a, r, "colaboracion.task", false); e != nil {
				return e
			}
			resource = &r
		} else if e := s.Core.Authorize(ctx, tx, a, "colaboracion.task", a.UserID, "", false); e != nil {
			return e
		}
		if e := s.validateTaskAssignee(ctx, tx, p.AssigneeID, resource); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.task.create", key, p, func() (any, error) {
			r, e := tx.ExecContext(ctx, `INSERT INTO collaboration_tasks(creator_id,assignee_id,resource_type,resource_id,title,note,due_date,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,'open',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, a.UserID, p.AssigneeID, nullableString(p.ResourceType), nullableString(p.ResourceID), p.Title, p.Note, nullableString(p.DueDate))
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			id := core.WrapID(n)
			if e = s.Core.Audit(ctx, tx, a, "collaboration.task_created", "collaboration_tasks", id, "success", map[string]string{"assignee_id": p.AssigneeID, "resource_type": p.ResourceType, "resource_id": p.ResourceID}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "collaboration.task_created", "collaboration_tasks", id, 1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": "1"}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) task(ctx context.Context, q core.Querier, a core.Actor, id string, lock bool) (Task, error) {
	var t Task
	if lock {
		tx, ok := q.(*sql.Tx)
		if !ok {
			return t, core.Invalid
		}
		if e := s.lockTaskParticipants(ctx, tx, a, id); e != nil {
			return t, e
		}
	}
	if e := s.authorizeAny(ctx, q, a, "colaboracion.view"); e != nil {
		return t, e
	}
	if a.Type != "user" || core.ID(id) != nil {
		return t, core.Missing
	}
	var rt, rid, due sql.NullString
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT CAST(t.id AS CHAR),CAST(t.creator_id AS CHAR),CAST(t.assignee_id AS CHAR),t.resource_type,CAST(t.resource_id AS CHAR),t.title,t.note,CAST(t.due_date AS CHAR),t.status,t.version,t.created_at,t.updated_at FROM collaboration_tasks t WHERE t.id=?`+suffix, id).Scan(&t.ID, &t.CreatorID, &t.AssigneeID, &rt, &rid, &t.Title, &t.Note, &due, &t.Status, &t.Version, &t.CreatedAt, &t.UpdatedAt)
	if e == sql.ErrNoRows {
		return Task{}, core.Missing
	}
	if e != nil {
		return Task{}, e
	}
	if a.UserID != t.CreatorID && a.UserID != t.AssigneeID {
		return Task{}, core.Missing
	}
	if e = q.QueryRowContext(ctx, `SELECT name FROM users WHERE id=?`, t.AssigneeID).Scan(&t.AssigneeName); e != nil {
		return Task{}, e
	}
	t.ResourceType, t.ResourceID, t.DueDate = rt.String, rid.String, due.String
	if t.ResourceType != "" {
		r, e := s.resource(ctx, q, a, t.ResourceType, t.ResourceID)
		if e != nil {
			return Task{}, core.Missing
		}
		t.ResourceLabel = r.Label
	}
	return t, nil
}

func (s *Service) Tasks(ctx context.Context, a core.Actor, f TaskFilter) (TaskPage, error) {
	out := TaskPage{Data: []Task{}, Page: f.Page, PerPage: f.PerPage}
	if !pageBounds(f.Page, f.PerPage) || (f.View != "" && f.View != "mine" && f.View != "today" && f.View != "overdue") || (f.Status != "" && !validTaskStatus(f.Status)) {
		return out, core.Invalid
	}
	if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.view"); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(t.id AS CHAR) FROM collaboration_tasks t WHERE t.creator_id=? OR t.assignee_id=? ORDER BY COALESCE(t.due_date,'9999-12-31'),t.updated_at DESC,t.id DESC`, a.UserID, a.UserID)
	if e != nil {
		return out, e
	}
	ids := []string{}
	for rows.Next() {
		var id string
		if e = rows.Scan(&id); e != nil {
			rows.Close()
			return out, e
		}
		ids = append(ids, id)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return out, e
	}
	today := taskToday()
	filtered := make([]Task, 0, len(ids))
	for _, id := range ids {
		t, e := s.task(ctx, s.Core.DB, a, id, false)
		if e == core.Missing || e == core.Forbidden {
			continue
		}
		if e != nil {
			return out, e
		}
		if f.Status != "" && t.Status != f.Status {
			continue
		}
		if f.View == "today" && (!activeTaskStatus(t.Status) || t.DueDate != today) {
			continue
		}
		if f.View == "overdue" && (!activeTaskStatus(t.Status) || t.DueDate == "" || t.DueDate >= today) {
			continue
		}
		filtered = append(filtered, t)
	}
	out.Total = len(filtered)
	start := (f.Page - 1) * f.PerPage
	if start >= len(filtered) {
		return out, nil
	}
	end := start + f.PerPage
	if end > len(filtered) {
		end = len(filtered)
	}
	out.Data = filtered[start:end]
	return out, nil
}

func (s *Service) UpdateTask(ctx context.Context, a core.Actor, id, key string, p TaskUpdate) (any, error) {
	p.Title, p.Note = strings.TrimSpace(p.Title), strings.TrimSpace(p.Note)
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) || core.ID(p.AssigneeID) != nil || !validName(p.Title, 180) || len([]rune(p.Note)) > 4000 || strings.ContainsRune(p.Note, '\x00') || !validDueDate(p.DueDate) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.lockTaskParticipants(ctx, tx, a, id, p.AssigneeID); e != nil {
			return e
		}
		t, e := s.task(ctx, tx, a, id, true)
		if e != nil {
			return e
		}
		if t.CreatorID != a.UserID {
			return core.Conflict
		}
		var resource *Resource
		if t.ResourceType != "" {
			r, e := s.resource(ctx, tx, a, t.ResourceType, t.ResourceID)
			if e != nil {
				return e
			}
			if e = resourcePermission(ctx, tx, s, a, r, "colaboracion.task", false); e != nil {
				return e
			}
			resource = &r
		} else if e := s.Core.Authorize(ctx, tx, a, "colaboracion.task", a.UserID, "", false); e != nil {
			return e
		}
		if e := s.validateTaskAssignee(ctx, tx, p.AssigneeID, resource); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.task.update:"+id, key, p, func() (any, error) {
			if t.Version != p.Version || !activeTaskStatus(t.Status) {
				return nil, core.Conflict
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_tasks SET assignee_id=?,title=?,note=?,due_date=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, p.AssigneeID, p.Title, p.Note, nullableString(p.DueDate), id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.task_updated", "collaboration_tasks", id, "success", map[string]string{"version": fmt.Sprint(p.Version + 1), "assignee_id": p.AssigneeID}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.task_updated", "collaboration_tasks", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1)}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) ChangeTaskStatus(ctx context.Context, a core.Actor, id, key string, p TaskStatusInput) (any, error) {
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) || !validTaskStatus(p.Status) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		t, e := s.task(ctx, tx, a, id, true)
		if e != nil {
			return e
		}
		if a.UserID != t.AssigneeID {
			return core.Forbidden
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.task.status:"+id, key, p, func() (any, error) {
			if t.Version != p.Version || !activeTaskStatus(t.Status) {
				return nil, core.Conflict
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_tasks SET status=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, p.Status, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.task_status_changed", "collaboration_tasks", id, "success", map[string]string{"status": p.Status, "version": fmt.Sprint(p.Version + 1)}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.task_status_changed", "collaboration_tasks", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1), "status": p.Status}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) ReopenTask(ctx context.Context, a core.Actor, id, key string, p TaskReopenInput) (any, error) {
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		t, e := s.task(ctx, tx, a, id, true)
		if e != nil {
			return e
		}
		if t.CreatorID != a.UserID {
			return core.Conflict
		}
		if t.ResourceType != "" {
			r, e := s.resource(ctx, tx, a, t.ResourceType, t.ResourceID)
			if e != nil {
				return e
			}
			if e = resourcePermission(ctx, tx, s, a, r, "colaboracion.task", true); e != nil {
				return e
			}
		} else if e := s.Core.Authorize(ctx, tx, a, "colaboracion.task", a.UserID, "", true); e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.task.reopen:"+id, key, p, func() (any, error) {
			if t.Version != p.Version || (t.Status != "done" && t.Status != "cancelled") {
				return nil, core.Conflict
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_tasks SET status='open',version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, id, p.Version)); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.task_reopened", "collaboration_tasks", id, "success", map[string]string{"version": fmt.Sprint(p.Version + 1)}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.task_reopened", "collaboration_tasks", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": fmt.Sprint(p.Version + 1), "status": "open"}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) TaskAssignees(ctx context.Context, a core.Actor, resourceType, resourceID, search string) ([]Assignee, error) {
	if a.Type != "user" || len([]rune(search)) > 100 || strings.ContainsRune(search, '\x00') || (resourceType == "") != (resourceID == "") || resourceID != "" && core.ID(resourceID) != nil {
		return nil, core.Invalid
	}
	var resource *Resource
	if resourceType != "" {
		r, e := s.resource(ctx, s.Core.DB, a, resourceType, resourceID)
		if e != nil {
			return nil, e
		}
		if e = resourcePermission(ctx, s.Core.DB, s, a, r, "colaboracion.task", false); e != nil {
			return nil, e
		}
		resource = &r
	} else if e := s.Core.Authorize(ctx, s.Core.DB, a, "colaboracion.task", a.UserID, "", false); e != nil {
		return nil, e
	}
	where := "active=TRUE AND totp_secret IS NOT NULL"
	args := []any{}
	if search != "" {
		where += " AND name LIKE ? ESCAPE '!'"
		args = append(args, "%"+strings.NewReplacer("!", "!!", "%", "!%", "_", "!_").Replace(search)+"%")
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(id AS CHAR),name FROM users WHERE `+where+` ORDER BY name,id LIMIT 200`, args...)
	if e != nil {
		return nil, e
	}
	candidates := []Assignee{}
	for rows.Next() {
		var item Assignee
		if e = rows.Scan(&item.ID, &item.Name); e != nil {
			rows.Close()
			return nil, e
		}
		candidates = append(candidates, item)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return nil, e
	}
	out := []Assignee{}
	for _, item := range candidates {
		if resource != nil {
			e = s.grantHolderCanResource(ctx, s.Core.DB, item.ID, *resource)
		} else {
			e = s.Core.AuthorizeGrantHolder(ctx, s.Core.DB, item.ID, "colaboracion.view", item.ID, "")
		}
		if e != nil {
			if e == core.Forbidden {
				continue
			}
			return nil, e
		}
		out = append(out, item)
		if len(out) == 50 {
			break
		}
	}
	return out, nil
}
