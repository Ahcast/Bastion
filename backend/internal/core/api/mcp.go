package api

import (
	"context"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"net/http"
	"strconv"
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
		mcp.AddTool(server, &mcp.Tool{Name: "core_document_download", Description: "Obtener acceso temporal a un documento privado autorizado y analizado."}, func(ctx context.Context, req *mcp.CallToolRequest, args struct {
			ID     string `json:"id"`
			Number int    `json:"number,omitempty"`
		}) (*mcp.CallToolResult, map[string]any, error) {
			if core.ID(args.ID) != nil || args.Number < 0 {
				return nil, nil, core.Invalid
			}
			v, e := s.Service.DownloadFile(ctx, a, args.ID, args.Number, s.Objects)
			if e != nil {
				return nil, nil, e
			}
			if e = s.Service.Audit(ctx, s.Service.DB, a, "mcp.core_document_download", "document", args.ID, "success", map[string]string{"number": strconv.Itoa(args.Number)}); e != nil {
				return nil, nil, e
			}
			return nil, v.(map[string]any), nil
		})
		s.Registry.Tools(server)
		return server
	}, &mcp.StreamableHTTPOptions{Stateless: true, JSONResponse: true})
	handler.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), contextKey{}, request{Actor: a, Session: core.Session{MFA: true}})))
}
