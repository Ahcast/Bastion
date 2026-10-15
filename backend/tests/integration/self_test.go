package integration

import (
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"testing"
)

func TestSelfPasswordRecoveryAndPollingExpiry(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	codes, e := s.RecoveryCodes(ctx, a)
	if e != nil || len(codes) != 8 {
		t.Fatal(e)
	}
	var n int
	_ = s.DB.QueryRow("SELECT COUNT(*) FROM recovery_codes WHERE user_id=1").Scan(&n)
	if n != 8 {
		t.Fatal(n)
	}
	next, e := s.RecoveryCodes(ctx, a)
	if e != nil || next[0] == codes[0] {
		t.Fatal(e)
	}
	if e = s.SavePreferences(ctx, a, core.Preferences{Theme: "dark", PageSize: 50}); e != nil {
		t.Fatal(e)
	}
	p, e := s.Preferences(ctx, a)
	if e != nil || p.(core.Preferences).Theme != "dark" {
		t.Fatal(e, p)
	}
	if e = s.ChangePassword(ctx, a, "different synthetic password 42"); e != nil {
		t.Fatal(e)
	}
	var secret, hash string
	if e = s.DB.QueryRow("SELECT totp_secret,password_hash FROM users WHERE id=1").Scan(&secret, &hash); e != nil {
		t.Fatal(e)
	}
	if secret == "" || !security.Verify("different synthetic password 42", hash) {
		t.Fatal("MFA reset or password not changed")
	}
	if e = s.Authorize(ctx, s.DB, a, "core.users.read", "", "", false); e != core.Unauthorized {
		t.Fatal("password did not revoke sessions", e)
	}
	session, _, e := s.NewSession(ctx, "1", true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	_, e = s.DB.Exec("UPDATE sessions SET last_activity=UTC_TIMESTAMP(6)-INTERVAL 31 MINUTE WHERE token_hash=?", security.Hash(session))
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Session(ctx, session); e != core.Unauthorized {
		t.Fatal("idle expiry", e)
	}
}

func TestAdministrativeIntegrationSwitchAndTypedSettings(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	if !s.IntegrationEnabled(ctx, "mcp") {
		t.Fatal("environment default")
	}
	value, _ := json.Marshal(core.Integrations{MCP: false, Files: true, Email: false})
	if e := s.SaveSetting(ctx, a, "integrations", core.Setting{Value: value}, ""); e != nil {
		t.Fatal(e)
	}
	if s.IntegrationEnabled(ctx, "mcp") || s.IntegrationEnabled(ctx, "email") || !s.IntegrationEnabled(ctx, "files") {
		t.Fatal("integration switch not effective")
	}
	if e := s.SaveSetting(ctx, a, "company", core.Setting{Value: json.RawMessage(`{"name":"Synthetic","password":"must not be stored"}`)}, ""); e != core.Invalid {
		t.Fatal("unknown sensitive configuration persisted", e)
	}
	s.Config.Files = false
	if s.IntegrationEnabled(ctx, "files") {
		t.Fatal("deployment ceiling bypassed")
	}
}
