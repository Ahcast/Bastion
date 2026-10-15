package integration

import (
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

func TestDelegatedUserManagerCannotTakeSystemAdministrator(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)

	created, err := s.SaveRole(ctx, a, "", core.Role{
		Name:   "Gestión de usuarios",
		Grants: []core.Grant{{Permission: "core.users.manage", Scope: "all"}},
	})
	if err != nil {
		t.Fatal(err)
	}
	roleID := created.(map[string]string)["id"]

	invite, err := s.CreateIdentityToken(ctx, a, "invite", "manager@example.test")
	if err != nil {
		t.Fatal(err)
	}
	inviteToken := invite.(map[string]any)["token"].(string)
	if err = s.AcceptIdentityToken(ctx, inviteToken, "Gestor sintético", "synthetic password manager 42"); err != nil {
		t.Fatal(err)
	}
	if err = s.UpdateUser(ctx, a, "2", core.UserChange{Name: "Gestor sintético", Active: true, Roles: []string{roleID}, Version: 1}); err != nil {
		t.Fatal(err)
	}

	encrypted, err := security.Encrypt(s.Config.Key, "JBSWY3DPEHPK3PXP")
	if err != nil {
		t.Fatal(err)
	}
	if _, err = s.DB.Exec("UPDATE users SET totp_secret=? WHERE id=2", encrypted); err != nil {
		t.Fatal(err)
	}
	session, _, err := s.NewSession(ctx, "2", true, "192.0.2.10")
	if err != nil {
		t.Fatal(err)
	}
	managerSessionHash := security.Hash(session)
	if _, err = s.DB.Exec("UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?", managerSessionHash); err != nil {
		t.Fatal(err)
	}
	manager := core.Actor{Type: "user", ID: "2", UserID: "2", SessionHash: managerSessionHash, Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}

	if _, err = s.CreateIdentityToken(ctx, manager, "reset", "admin@example.test"); err != core.Forbidden {
		t.Fatalf("administrator password reset token was allowed: %v", err)
	}
	if err = s.ResetMFA(ctx, manager, "1", "verified-personally-001"); err != core.Forbidden {
		t.Fatalf("administrator MFA reset was allowed: %v", err)
	}
	if err = s.RevokeSessions(ctx, manager, "1"); err != core.Forbidden {
		t.Fatalf("administrator session revocation was allowed: %v", err)
	}

	// A reset token issued before this hardening must not remain usable after
	// deployment, otherwise an already-issued token could complete the old A2 chain.
	stale := security.Token()
	if _, err = s.DB.Exec(
		"INSERT INTO identity_tokens(token_hash,kind,email,user_id,created_by,expires_at) VALUES(?,?,?,?,?,?)",
		security.Hash(stale), "reset", "admin@example.test", "1", "2", time.Now().UTC().Add(time.Hour),
	); err != nil {
		t.Fatal(err)
	}
	if err = s.AcceptIdentityToken(ctx, stale, "", "synthetic password replacement 42"); err != core.Forbidden {
		t.Fatalf("pre-existing administrator reset token was accepted: %v", err)
	}
}
