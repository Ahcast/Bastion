package integration

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

type bearer struct{ token string }

func (b bearer) RoundTrip(r *http.Request) (*http.Response, error) {
	c := r.Clone(r.Context())
	c.Header.Set("Authorization", "Bearer "+b.token)
	return http.DefaultTransport.RoundTrip(c)
}
func TestMCPStreamableSDKMatchesRESTPolicy(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	v, e := s.CreateService(ctx, a, "SDK Test", []string{"core.modules.read"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	token := v.(map[string]any)["token"].(string)
	r := registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Plataforma", Version: "0.1.0", Enabled: true})
	h := api.New(s, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	server := httptest.NewServer(h)
	defer server.Close()
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	connectCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	session, e := client.Connect(connectCtx, &mcp.StreamableClientTransport{Endpoint: server.URL + "/mcp", HTTPClient: &http.Client{Transport: bearer{token}}}, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer session.Close()
	result, e := session.CallTool(connectCtx, &mcp.CallToolParams{Name: "core_modules", Arguments: map[string]any{}})
	if e != nil || result.IsError {
		t.Fatal(e, result)
	}
	denied, e := session.CallTool(connectCtx, &mcp.CallToolParams{Name: "core_jobs", Arguments: map[string]any{}})
	if e == nil && !denied.IsError {
		t.Fatal("MCP scope bypass")
	}
	actor, e := s.AuthenticateService(ctx, token)
	if e != nil {
		t.Fatal(e)
	}
	if e = s.RevokeService(ctx, a, actor.ID); e != nil {
		t.Fatal(e)
	}
	_, e = session.CallTool(connectCtx, &mcp.CallToolParams{Name: "core_modules", Arguments: map[string]any{}})
	if e == nil {
		t.Fatal("revoked MCP connection accepted")
	}
}
