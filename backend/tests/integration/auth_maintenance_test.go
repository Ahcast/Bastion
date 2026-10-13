package integration

import (
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

func TestAuthMaintenanceRemovesOnlyExpiredState(t *testing.T) {
	s := fixture(t)
	now := time.Now().UTC().Truncate(time.Microsecond)

	type sessionSeed struct {
		name    string
		last    time.Time
		expires time.Time
	}
	sessions := []sessionSeed{
		{name: "absolute-expired", last: now, expires: now.Add(-time.Minute)},
		{name: "idle-expired", last: now.Add(-s.Config.Idle - time.Minute), expires: now.Add(time.Hour)},
		{name: "active", last: now.Add(-s.Config.Idle + 5*time.Minute), expires: now.Add(time.Hour)},
	}
	for _, seed := range sessions {
		if _, err := s.DB.Exec(
			"INSERT INTO sessions(token_hash,csrf_hash,user_id,mfa_complete,created_at,last_activity,expires_at) VALUES(?,?,NULL,FALSE,?,?,?)",
			security.Hash("session:"+seed.name), security.Hash("csrf:"+seed.name), now.Add(-time.Hour), seed.last, seed.expires,
		); err != nil {
			t.Fatal(err)
		}
	}

	oldLimit := security.Hash("limit:old")
	currentLimit := security.Hash("limit:current")
	if _, err := s.DB.Exec(
		"INSERT INTO auth_limits(bucket,attempts,window_start) VALUES(?,?,?),(?,?,?)",
		oldLimit, 7, now.Add(-16*time.Minute), currentLimit, 4, now.Add(-14*time.Minute),
	); err != nil {
		t.Fatal(err)
	}

	cleaned, err := s.CleanupExpiredAuthState(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if cleaned.Sessions != 2 || cleaned.AuthLimits != 1 {
		t.Fatalf("unexpected cleanup result: %+v", cleaned)
	}

	var count int
	if err = s.DB.QueryRow("SELECT COUNT(*) FROM sessions").Scan(&count); err != nil || count != 1 {
		t.Fatal("invalid session cleanup", count, err)
	}
	if err = s.DB.QueryRow("SELECT COUNT(*) FROM sessions WHERE token_hash=?", security.Hash("session:active")).Scan(&count); err != nil || count != 1 {
		t.Fatal("active session removed", count, err)
	}
	if err = s.DB.QueryRow("SELECT COUNT(*) FROM auth_limits").Scan(&count); err != nil || count != 1 {
		t.Fatal("invalid auth limit cleanup", count, err)
	}
	var attempts int
	if err = s.DB.QueryRow("SELECT attempts FROM auth_limits WHERE bucket=?", currentLimit).Scan(&attempts); err != nil || attempts != 4 {
		t.Fatal("current auth limit changed", attempts, err)
	}

	cleaned, err = s.CleanupExpiredAuthState(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if cleaned.Sessions != 0 || cleaned.AuthLimits != 0 {
		t.Fatalf("cleanup is not idempotent: %+v", cleaned)
	}
}
