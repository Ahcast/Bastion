package ai

import (
	"context"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"strings"
	"testing"
)

func TestExecutionToolsPublishStringVersionsAndRejectActorOverride(t *testing.T) {
	server := mcp.NewServer(&mcp.Implementation{Name: "schema-test", Version: "1"}, nil)
	s := &Service{}
	s.tools(server)
	client := mcp.NewClient(&mcp.Implementation{Name: "schema-client", Version: "1"}, nil)
	one, two := mcp.NewInMemoryTransports()
	ctx := context.Background()
	ss, e := server.Connect(ctx, one, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer ss.Close()
	cs, e := client.Connect(ctx, two, nil)
	if e != nil {
		t.Fatal(e)
	}
	defer cs.Close()
	tools, e := cs.ListTools(ctx, &mcp.ListToolsParams{})
	if e != nil {
		t.Fatal(e)
	}
	for _, tool := range tools.Tools {
		if tool.Name != "renew_ai_task" && tool.Name != "fail_ai_task" {
			continue
		}
		schema := tool.InputSchema.(map[string]any)
		props := schema["properties"].(map[string]any)
		for _, name := range []string{"version", "input_version", "run_id", "task_id"} {
			if props[name].(map[string]any)["type"] != "string" {
				t.Fatal(tool.Name, name)
			}
		}
	}
	args := map[string]any{"task_id": "1", "idempotency_key": "synthetic-key", "schema_version": "1.0", "version": "1", "run_id": "1", "input_version": "1", "input_hash": strings.Repeat("a", 64), "skill_version": "v1", "lease_token": strings.Repeat("a", 43)}
	result, e := cs.CallTool(ctx, &mcp.CallToolParams{Name: "renew_ai_task", Arguments: args})
	// A valid schema reaches Core's absent-authentication denial without a DB.
	if e != nil || !result.IsError {
		t.Fatal(result, e)
	}
	b := result.Content[0].(*mcp.TextContent).Text
	if !strings.Contains(b, "unauthenticated") {
		t.Fatal("string schema mismatch", b)
	}
	args["requested_by"] = "1"
	result, e = cs.CallTool(ctx, &mcp.CallToolParams{Name: "renew_ai_task", Arguments: args})
	if e == nil && !result.IsError {
		t.Fatal("actor override schema accepted")
	}
}
