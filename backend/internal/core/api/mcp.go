package api

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"net/http"
	"strings"
)

func (s *Server) serveMCP(w http.ResponseWriter, r *http.Request, id string) {
	if !s.Service.IntegrationEnabled(r.Context(), "mcp") {
		s.error(w, r, core.Disabled)
		return
	}
	header := r.Header.Get("Authorization")
	if !strings.HasPrefix(header, "Bearer ") {
		s.error(w, r, core.Unauthorized)
		return
	}
	a, e := s.Service.AuthenticateService(r.Context(), strings.TrimPrefix(header, "Bearer "))
	if e != nil {
		s.error(w, r, e)
		return
	}
	a.Source = "mcp"
	a.RequestID = id
	a.CorrelationID = id
	handler := mcp.NewStreamableHTTPHandler(func(_ *http.Request) *mcp.Server {
		server := mcp.NewServer(&mcp.Implementation{Name: "bastion-core", Version: "0.1.0"}, nil)
		mcp.AddTool(server, &mcp.Tool{Name: "core_modules", Description: "Consultar los módulos implementados y habilitados de Bastion."}, func(ctx context.Context, req *mcp.CallToolRequest, args struct{}) (*mcp.CallToolResult, map[string]any, error) {
			v, e := s.Modules(ctx, a)
			if e != nil {
				return nil, nil, e
			}
			if e = s.Service.Audit(ctx, s.Service.DB, a, "mcp.core_modules", "module", "", "success", map[string]string{}); e != nil {
				return nil, nil, e
			}
			return nil, v.(map[string]any), nil
		})
		mcp.AddTool(server, &mcp.Tool{Name: "core_jobs", Description: "Consultar hasta cien trabajos técnicos sin payloads ni secretos."}, func(ctx context.Context, req *mcp.CallToolRequest, args struct{}) (*mcp.CallToolResult, map[string]any, error) {
			v, e := s.Service.Jobs(ctx, a)
			if e != nil {
				return nil, nil, e
			}
			if e = s.Service.Audit(ctx, s.Service.DB, a, "mcp.core_jobs", "job", "", "success", map[string]string{}); e != nil {
				return nil, nil, e
			}
			return nil, v.(map[string]any), nil
		})

		// Private document delivery is deliberately not an MCP capability. A
		// service identity must never receive presigned S3 download URLs.
		// Module tools remain registered, but Authorize applies the explicit
		// service-permission allowlist before any underlying user grant.
		s.Registry.Tools(server)
		return server
	}, &mcp.StreamableHTTPOptions{Stateless: true, JSONResponse: true})
	handler.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), contextKey{}, request{Actor: a, Session: core.Session{MFA: true}})))
}
