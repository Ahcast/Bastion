package integration

import (
	"database/sql"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

func TestScheduledOutboxPreservesDueTimeWithConcurrentPublication(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	due := time.Now().In(time.FixedZone("Tegucigalpa", -6*3600)).Add(time.Hour).Truncate(time.Microsecond)
	var id string
	if err := s.Transact(ctx, func(tx *sql.Tx) error {
		var err error
		id, err = s.EnqueueAt(ctx, tx, a, "notification", "core.notifications.read", map[string]string{"message": "Scheduled synthetic notification"}, due)
		return err
	}); err != nil {
		t.Fatal(err)
	}
	var wg sync.WaitGroup
	errs := make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); errs <- s.PublishOutbox(ctx) }()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Fatal(err)
		}
	}
	var actual time.Time
	var count int
	if err := s.DB.QueryRow("SELECT available_at FROM jobs WHERE id=?", id).Scan(&actual); err != nil || !actual.Equal(due) {
		t.Fatal("timezone or publication changed schedule", actual, due, err)
	}
	if err := s.DB.QueryRow("SELECT COUNT(*) FROM jobs WHERE id=?", id).Scan(&count); err != nil || count != 1 {
		t.Fatal("duplicate job", count, err)
	}
	if _, err := s.ClaimJob(ctx); err != sql.ErrNoRows {
		t.Fatal("future job claimed", err)
	}
	if _, err := s.DB.Exec("UPDATE jobs SET available_at=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE id=?", id); err != nil {
		t.Fatal(err)
	}
	j, err := s.ClaimJob(ctx)
	if err != nil || j.ID != id {
		t.Fatal("due job not claimed", err)
	}
	if err = s.NotificationHandler(ctx, j); err != nil {
		t.Fatal(err)
	}
	if err = s.FinishJob(ctx, j, "completed", "", time.Now()); err != nil {
		t.Fatal(err)
	}
	if err = s.PublishOutbox(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err = s.ClaimJob(ctx); err != sql.ErrNoRows {
		t.Fatal("completed schedule republished", err)
	}
	if err = s.DB.QueryRow("SELECT COUNT(*) FROM notifications").Scan(&count); err != nil || count != 1 {
		t.Fatal("scheduled effect duplicated", count, err)
	}
}

func TestScheduledOutboxLatePublisherAndLegacyEvent(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	due := time.Now().UTC().Add(-time.Hour).Truncate(time.Microsecond)
	var late string
	if err := s.Transact(ctx, func(tx *sql.Tx) error {
		var err error
		late, err = s.EnqueueAt(ctx, tx, a, "notification", "core.notifications.read", map[string]string{"message": "Late publisher"}, due)
		return err
	}); err != nil {
		t.Fatal(err)
	}
	// Represents an event created before migration 0011, with no schedule.
	if _, err := s.DB.Exec("INSERT INTO outbox_events(id,kind,payload,user_id,permission,correlation_id,created_at) VALUES('0123456789abcdef0123456789abcdef','notification','{\"message\":\"Legacy event\"}',1,'core.notifications.read','synthetic',UTC_TIMESTAMP(6)-INTERVAL 2 HOUR)"); err != nil {
		t.Fatal(err)
	}
	if err := s.PublishOutbox(ctx); err != nil {
		t.Fatal(err)
	}
	j, err := s.ClaimJob(ctx)
	if err != nil || j.ID != "0123456789abcdef0123456789abcdef" {
		t.Fatal("legacy immediate event changed", err)
	}
	if err = s.FinishJob(ctx, j, "completed", "", time.Now()); err != nil {
		t.Fatal(err)
	}
	j, err = s.ClaimJob(ctx)
	if err != nil || j.ID != late {
		t.Fatal("overdue event not immediately available", err)
	}
	var actual time.Time
	if err = s.DB.QueryRow("SELECT available_at FROM jobs WHERE id=?", late).Scan(&actual); err != nil || !actual.Equal(due) {
		t.Fatal("late publisher reset schedule", err)
	}
}

func TestScheduledOutboxRollbackInvalidDateAndRevocation(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	future := time.Now().Add(time.Hour)
	if err := s.Transact(ctx, func(tx *sql.Tx) error {
		if _, err := s.EnqueueAt(ctx, tx, a, "notification", "core.notifications.read", map[string]string{"message": "Never committed"}, future); err != nil {
			return err
		}
		return errors.New("synthetic rollback")
	}); err == nil {
		t.Fatal("rollback missing")
	}
	for _, invalid := range []time.Time{{}, time.Date(999, 12, 31, 0, 0, 0, 0, time.UTC), time.Date(10000, 1, 1, 0, 0, 0, 0, time.UTC)} {
		if err := s.Transact(ctx, func(tx *sql.Tx) error {
			_, err := s.EnqueueAt(ctx, tx, a, "notification", "core.notifications.read", nil, invalid)
			return err
		}); err != core.Invalid {
			t.Fatal("unsupported MySQL date accepted", err)
		}
	}
	if _, err := s.DB.Exec("DELETE FROM role_permissions WHERE permission='core.notifications.read'"); err != nil {
		t.Fatal(err)
	}
	if err := s.Transact(ctx, func(tx *sql.Tx) error {
		_, err := s.EnqueueAt(ctx, tx, a, "notification", "core.notifications.read", nil, future)
		return err
	}); err != core.Forbidden {
		t.Fatal("scheduled enqueue bypassed live permission", err)
	}
	if err := s.PublishOutbox(ctx); err != nil {
		t.Fatal(err)
	}
	var count int
	if err := s.DB.QueryRow("SELECT COUNT(*) FROM jobs").Scan(&count); err != nil || count != 0 {
		t.Fatal("rejected/rolled-back schedule persisted", count, err)
	}
}
