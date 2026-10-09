package core

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"time"
)

var ErrUncertain = errors.New("external_result_uncertain")

type Job struct {
	ID, Kind, UserID, Permission, CorrelationID, Lease string
	Payload                                            json.RawMessage
	Attempts                                           int
}
type JobHandler func(context.Context, Job) error
type Worker struct {
	Service  *Service
	Handlers map[string]JobHandler
}

func (s *Service) Enqueue(ctx context.Context, tx *sql.Tx, a Actor, kind, permission string, payload any) (string, error) {
	if e := s.Authorize(ctx, tx, a, permission, a.UserID, "", false); e != nil {
		return "", e
	}
	b, e := json.Marshal(payload)
	if e != nil {
		return "", e
	}
	id := security.ID()
	_, e = tx.ExecContext(ctx, `INSERT INTO outbox_events(id,kind,payload,user_id,permission,correlation_id,created_at) VALUES(?,?,?,?,?,?,UTC_TIMESTAMP(6))`, id, kind, string(b), a.UserID, permission, a.CorrelationID)
	return id, e
}
func (s *Service) PublishOutbox(ctx context.Context) error {
	return s.Transact(ctx, func(tx *sql.Tx) error {
		rows, e := tx.QueryContext(ctx, "SELECT id,kind,payload,user_id,permission,correlation_id FROM outbox_events WHERE published_at IS NULL ORDER BY created_at,id LIMIT 20 FOR UPDATE")
		if e != nil {
			return e
		}
		var events []Job
		for rows.Next() {
			var j Job
			if e = rows.Scan(&j.ID, &j.Kind, &j.Payload, &j.UserID, &j.Permission, &j.CorrelationID); e != nil {
				rows.Close()
				return e
			}
			events = append(events, j)
		}
		e = rows.Err()
		rows.Close()
		if e != nil {
			return e
		}
		for _, j := range events {
			if _, e = tx.ExecContext(ctx, `INSERT IGNORE INTO jobs(id,kind,payload,user_id,permission,correlation_id,status,available_at,created_at) VALUES(?,?,?,?,?,?,'pending',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, j.ID, j.Kind, string(j.Payload), j.UserID, j.Permission, j.CorrelationID); e != nil {
				return e
			}
			if _, e = tx.ExecContext(ctx, "UPDATE outbox_events SET published_at=UTC_TIMESTAMP(6) WHERE id=?", j.ID); e != nil {
				return e
			}
		}
		return nil
	})
}
func (s *Service) ClaimJob(ctx context.Context) (Job, error) {
	var j Job
	err := s.Transact(ctx, func(tx *sql.Tx) error {
		e := tx.QueryRowContext(ctx, `SELECT id,kind,payload,user_id,permission,correlation_id,attempts FROM jobs WHERE (status='pending' AND available_at<=UTC_TIMESTAMP(6)) OR (status='running' AND lease_until<UTC_TIMESTAMP(6)) ORDER BY available_at,id LIMIT 1 FOR UPDATE`).Scan(&j.ID, &j.Kind, &j.Payload, &j.UserID, &j.Permission, &j.CorrelationID, &j.Attempts)
		if e != nil {
			return e
		}
		j.Lease = security.ID()
		j.Attempts++
		_, e = tx.ExecContext(ctx, "UPDATE jobs SET status='running',attempts=?,lease_token=?,lease_until=? WHERE id=?", j.Attempts, j.Lease, time.Now().UTC().Add(90*time.Second), j.ID)
		return e
	})
	return j, err
}

// Permission reevaluation uses current user grants, never the permissions captured when queued.
func (s *Service) JobAllowed(ctx context.Context, j Job) error {
	var enabled bool
	var secret sql.NullString
	if e := s.DB.QueryRowContext(ctx, "SELECT active,totp_secret FROM users WHERE id=?", j.UserID).Scan(&enabled, &secret); e != nil {
		return e
	}
	if !enabled || !secret.Valid {
		return Forbidden
	}
	grants, e := s.Grants(ctx, s.DB, j.UserID)
	if e != nil {
		return e
	}
	for _, g := range grants {
		if g.Permission == j.Permission && (g.Scope == "all" || (j.Permission == "core.notifications.read" && g.Scope == "self")) {
			return nil
		}
	}
	return Forbidden
}
func (w *Worker) Run(ctx context.Context, max int) error {
	if max < 1 || max > 100 {
		return Invalid
	}
	if e := w.Service.PublishOutbox(ctx); e != nil {
		return e
	}
	for i := 0; i < max; i++ {
		if ctx.Err() != nil {
			return ctx.Err()
		}
		j, e := w.Service.ClaimJob(ctx)
		if e == sql.ErrNoRows {
			return nil
		}
		if e != nil {
			return e
		}
		runCtx, cancel := context.WithTimeout(ctx, 45*time.Second)
		e = w.Service.JobAllowed(runCtx, j)
		if e == nil {
			h, ok := w.Handlers[j.Kind]
			if !ok {
				e = &Error{"unknown_job", "No existe un ejecutor para este trabajo.", 422}
			} else {
				e = h(runCtx, j)
			}
		}
		cancel()
		status := "completed"
		code := ""
		when := time.Now().UTC()
		if e != nil {
			code = "job_failed"
			status = "pending"
			when = when.Add(time.Duration(1<<min(j.Attempts, 8)) * time.Minute)
			if errors.Is(e, ErrUncertain) {
				status = "uncertain"
				code = "external_result_uncertain"
			} else if IsClient(e) || j.Attempts >= 5 {
				status = "failed"
				code = "job_rejected"
			}
		}
		if e = w.Service.FinishJob(ctx, j, status, code, when); e != nil {
			return e
		}
	}
	return nil
}
func (s *Service) FinishJob(ctx context.Context, j Job, status, code string, available time.Time) error {
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := Changed(tx.ExecContext(ctx, "UPDATE jobs SET status=?,last_error=?,available_at=?,lease_token=NULL,lease_until=NULL WHERE id=? AND lease_token=? AND lease_until>UTC_TIMESTAMP(6)", status, nullable(code), available, j.ID, j.Lease)); e != nil {
			return e
		}
		a := Actor{Type: "user", ID: j.UserID, UserID: j.UserID, Source: "job", RequestID: security.ID(), CorrelationID: j.CorrelationID}
		return s.Audit(ctx, tx, a, "jobs.finish", "job", j.ID, status, map[string]string{"code": code})
	})
}
func (s *Service) Jobs(ctx context.Context, a Actor) (any, error) {
	if e := s.Authorize(ctx, s.DB, a, "core.jobs.read", "", "", false); e != nil {
		return nil, e
	}
	return s.Rows(ctx, "SELECT id,kind,status,attempts,available_at,lease_until,last_error,created_at FROM jobs ORDER BY created_at DESC LIMIT 100")
}
func (s *Service) RetryJob(ctx context.Context, a Actor, id string) error {
	if len(id) != 32 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.jobs.retry", "", "", true); e != nil {
			return e
		}
		var status, kind string
		if e := tx.QueryRowContext(ctx, "SELECT status,kind FROM jobs WHERE id=? FOR UPDATE", id).Scan(&status, &kind); e != nil {
			return Missing
		}
		if status != "failed" || kind == "email.test" {
			return Conflict
		}
		if _, e := tx.ExecContext(ctx, "UPDATE jobs SET status='pending',attempts=0,available_at=UTC_TIMESTAMP(6),last_error=NULL WHERE id=?", id); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "jobs.retry", "job", id, "success", map[string]string{})
	})
}
func (s *Service) NotificationHandler(ctx context.Context, j Job) error {
	var p struct {
		Message string `json:"message"`
	}
	if e := json.Unmarshal(j.Payload, &p); e != nil || p.Message == "" || len(p.Message) > 500 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		var lease string
		if e := tx.QueryRowContext(ctx, "SELECT lease_token FROM jobs WHERE id=? AND lease_until>UTC_TIMESTAMP(6) FOR UPDATE", j.ID).Scan(&lease); e != nil || lease != j.Lease {
			return Conflict
		}
		var exists int
		if e := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM audit_events WHERE action='notification.create' AND entity_id=?", j.ID).Scan(&exists); e != nil {
			return e
		}
		if exists > 0 {
			return nil
		}
		if _, e := tx.ExecContext(ctx, "INSERT INTO notifications(user_id,message,created_at) VALUES(?,?,UTC_TIMESTAMP(6))", j.UserID, p.Message); e != nil {
			return e
		}
		return s.Audit(ctx, tx, Actor{Type: "user", ID: j.UserID, UserID: j.UserID, Source: "job", RequestID: security.ID(), CorrelationID: j.CorrelationID}, "notification.create", "job", j.ID, "success", map[string]string{})
	})
}
func (s *Service) QueueNotification(ctx context.Context, tx *sql.Tx, a Actor, message string) (string, error) {
	return s.Enqueue(ctx, tx, a, "notification", "core.notifications.read", map[string]string{"message": message})
}
func (s *Service) ReconcileEmail(ctx context.Context, a Actor, id, status, evidence string) error {
	if len(id) != 32 || (status != "accepted" && status != "failed") || len(evidence) < 10 || len(evidence) > 200 {
		return Invalid
	}
	return s.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Authorize(ctx, tx, a, "core.email.test", "", "", true); e != nil {
			return e
		}
		if e := Changed(tx.ExecContext(ctx, "UPDATE email_messages SET status=?,error_code=NULL,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND status IN ('sending','uncertain')", status, id)); e != nil {
			return e
		}
		return s.Audit(ctx, tx, a, "email.reconcile", "email", id, status, map[string]string{"evidence_reference": evidence})
	})
}
func (s *Service) CheckJobLease(ctx context.Context, j Job) error {
	var n int
	if e := s.DB.QueryRowContext(ctx, "SELECT COUNT(*) FROM jobs WHERE id=? AND status='running' AND lease_token=? AND lease_until>UTC_TIMESTAMP(6)", j.ID, j.Lease).Scan(&n); e != nil {
		return e
	}
	if n != 1 {
		return fmt.Errorf("lease expired")
	}
	return nil
}
