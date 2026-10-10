package core

import (
	"context"
	"database/sql"
	"encoding/json"
)

type Integrations struct {
	MCP   bool `json:"mcp"`
	Files bool `json:"files"`
	Email bool `json:"email"`
}

// Deployment flags are an upper bound; administrative configuration can disable integrations immediately.
func (s *Service) IntegrationEnabled(ctx context.Context, name string) bool {
	return s.IntegrationEnabledUsing(ctx, s.DB, name)
}
func (s *Service) IntegrationEnabledUsing(ctx context.Context, q Querier, name string) bool {
	ceiling := map[string]bool{"mcp": s.Config.MCP, "files": s.Config.Files, "email": s.Config.Email}
	if !ceiling[name] {
		return false
	}
	var raw string
	e := q.QueryRowContext(ctx, "SELECT value FROM settings WHERE name='integrations'").Scan(&raw)
	if e == sql.ErrNoRows {
		return true
	}
	if e != nil {
		return false
	}
	var value Integrations
	if json.Unmarshal([]byte(raw), &value) != nil {
		return false
	}
	return map[string]bool{"mcp": value.MCP, "files": value.Files, "email": value.Email}[name]
}
