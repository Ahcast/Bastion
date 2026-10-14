package integration

import (
	"encoding/json"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// Simulate a token issued before the service-read restriction. New ordinary
// business scopes must be rejected, and stored legacy scopes must not bypass it.
func deniedLegacyBusinessToken(t *testing.T, s *core.Service, actor core.Actor, scope string) string {
	t.Helper()
	if _, err := s.CreateService(ctx, actor, "Denied business scope", []string{scope}, time.Now().Add(time.Hour)); err != core.Forbidden {
		t.Fatalf("ordinary business permission became a service scope: %v", err)
	}
	token := security.Token()
	scopes, err := json.Marshal([]string{scope})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.DB.Exec(`INSERT INTO service_identities(name,token_hash,user_id,scopes,active,expires_at) VALUES(?,?,?,?,TRUE,?)`, "Legacy business fixture", security.Hash(token), actor.UserID, string(scopes), time.Now().Add(time.Hour)); err != nil {
		t.Fatal(err)
	}
	return token
}

func requireMCPBusinessDenied(t *testing.T, result *mcp.CallToolResult, err error) {
	t.Helper()
	if err != nil {
		t.Fatalf("MCP transport failed before authorization: %v", err)
	}
	if result == nil || !result.IsError {
		t.Fatalf("service token read human business data: %v", result)
	}
}
