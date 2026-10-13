package collaboration

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"strconv"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
)

func decodeRequired(r *http.Request, dst any, required ...string) error {
	const maxBody = 1 << 20
	body, e := io.ReadAll(io.LimitReader(r.Body, maxBody+1))
	if e != nil {
		return core.Invalid
	}
	if len(body) > maxBody {
		return core.Invalid
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(body, &fields) != nil || fields == nil {
		return core.Invalid
	}
	for _, name := range required {
		v, ok := fields[name]
		if !ok || bytes.Equal(bytes.TrimSpace(v), []byte("null")) {
			return core.Invalid
		}
	}
	r.Body = io.NopCloser(bytes.NewReader(body))
	return api.Decode(r, dst)
}

func queryValues(r *http.Request, allowed ...string) (map[string]string, error) {
	query := r.URL.Query()
	permitted := make(map[string]bool, len(allowed))
	for _, name := range allowed {
		permitted[name] = true
	}
	values := map[string]string{}
	for name, items := range query {
		if !permitted[name] || len(items) != 1 {
			return nil, core.Invalid
		}
		values[name] = items[0]
	}
	return values, nil
}

func queryBool(v map[string]string, key string) (bool, error) {
	x, ok := v[key]
	if !ok {
		return false, nil
	}
	if x != "true" && x != "false" {
		return false, core.Invalid
	}
	return x == "true", nil
}

func queryInt(v map[string]string, key string, fallback int) (int, error) {
	x, ok := v[key]
	if !ok {
		return fallback, nil
	}
	n, e := strconv.Atoi(x)
	if e != nil {
		return 0, core.Invalid
	}
	return n, nil
}

func (s *Service) Module(logger *slog.Logger, enabled bool, objects core.Objects) registry.Module {
	return registry.Module{ID: "collaboration", Label: "Colaboración", Version: "0.1.0", Enabled: enabled, Dependencies: []string{"core"}, Permissions: Permissions, Routes: func(mux *http.ServeMux) {
		register := func(pattern string, fn api.Action) {
			mux.Handle(pattern, api.Operation(s.Core, logger, fn))
		}
		register("GET /api/v1/collaboration", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "scope", "person_id", "folder_id", "search", "page", "per_page", "deleted")
			if e != nil {
				return nil, e
			}
			page, e := queryInt(q, "page", 1)
			if e != nil {
				return nil, e
			}
			perPage, e := queryInt(q, "per_page", 25)
			if e != nil {
				return nil, e
			}
			deleted, e := queryBool(q, "deleted")
			if e != nil {
				return nil, e
			}
			f := Filter{Scope: q["scope"], PersonID: q["person_id"], FolderID: q["folder_id"], FolderSet: hasQuery(q, "folder_id"), Search: q["search"], Deleted: deleted, Page: page, PerPage: perPage}
			return s.List(ctx, a, f)
		})
		register("GET /api/v1/collaboration/folders", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "scope", "person_id", "parent_id", "deleted")
			if e != nil {
				return nil, e
			}
			deleted, e := queryBool(q, "deleted")
			if e != nil {
				return nil, e
			}
			folders, e := s.ListFolders(ctx, a, q["scope"], q["person_id"], q["parent_id"], deleted)
			if e != nil {
				return nil, e
			}
			return map[string]any{"data": folders}, nil
		})
		register("GET /api/v1/collaboration/teams", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if _, e := queryValues(r); e != nil {
				return nil, e
			}
			teams, e := s.Teams(ctx, a)
			if e != nil {
				return nil, e
			}
			return map[string]any{"data": teams}, nil
		})

		// U3: Collaboration-owned task work. These routes never mutate Workflow.
		register("GET /api/v1/collaboration/tasks", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "view", "status", "page", "per_page")
			if e != nil {
				return nil, e
			}
			page, e := queryInt(q, "page", 1)
			if e != nil {
				return nil, e
			}
			perPage, e := queryInt(q, "per_page", 25)
			if e != nil {
				return nil, e
			}
			return s.Tasks(ctx, a, TaskFilter{View: q["view"], Status: q["status"], Page: page, PerPage: perPage})
		})
		register("GET /api/v1/collaboration/task-assignees", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "resource_type", "resource_id", "search")
			if e != nil {
				return nil, e
			}
			users, e := s.TaskAssignees(ctx, a, q["resource_type"], q["resource_id"], q["search"])
			if e != nil {
				return nil, e
			}
			return map[string]any{"data": users}, nil
		})
		register("POST /api/v1/collaboration/tasks", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p TaskInput
			if e := decodeRequired(r, &p, "assignee_id", "resource_type", "resource_id", "title", "note", "due_date"); e != nil {
				return nil, e
			}
			return s.CreateTask(ctx, a, r.Header.Get("Idempotency-Key"), p)
		})
		register("PATCH /api/v1/collaboration/tasks/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p TaskUpdate
			if e := decodeRequired(r, &p, "version", "assignee_id", "title", "note", "due_date"); e != nil {
				return nil, e
			}
			return s.UpdateTask(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("POST /api/v1/collaboration/tasks/{id}/status", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p TaskStatusInput
			if e := decodeRequired(r, &p, "version", "status"); e != nil {
				return nil, e
			}
			return s.ChangeTaskStatus(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("POST /api/v1/collaboration/tasks/{id}/reopen", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p TaskReopenInput
			if e := decodeRequired(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.ReopenTask(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})

		// U4: links are documentary only; approvals are a Workflow-owned read projection.
		register("GET /api/v1/collaboration/approvals", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "page", "per_page")
			if e != nil {
				return nil, e
			}
			page, e := queryInt(q, "page", 1)
			if e != nil {
				return nil, e
			}
			perPage, e := queryInt(q, "per_page", 25)
			if e != nil {
				return nil, e
			}
			return s.PendingApprovals(ctx, a, page, perPage)
		})
		register("GET /api/v1/collaboration/{id}/links", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if _, e := queryValues(r); e != nil {
				return nil, e
			}
			links, e := s.Links(ctx, a, r.PathValue("id"))
			if e != nil {
				return nil, e
			}
			return map[string]any{"data": links}, nil
		})
		register("PUT /api/v1/collaboration/{id}/links", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p SetLinksInput
			if e := decodeRequired(r, &p, "version", "links"); e != nil {
				return nil, e
			}
			return s.SetLinks(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})

		register("GET /api/v1/collaboration/resources/{resource_type}/{resource_id}/comments", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "page", "per_page")
			if e != nil {
				return nil, e
			}
			page, e := queryInt(q, "page", 1)
			if e != nil {
				return nil, e
			}
			perPage, e := queryInt(q, "per_page", 25)
			if e != nil {
				return nil, e
			}
			return s.Comments(ctx, a, r.PathValue("resource_type"), r.PathValue("resource_id"), page, perPage)
		})
		register("POST /api/v1/collaboration/resources/{resource_type}/{resource_id}/comments", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p CommentInput
			if e := decodeRequired(r, &p, "body", "mention_user_ids"); e != nil {
				return nil, e
			}
			return s.CreateComment(ctx, a, r.PathValue("resource_type"), r.PathValue("resource_id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("GET /api/v1/collaboration/resources/{resource_type}/{resource_id}/activity", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "page", "per_page")
			if e != nil {
				return nil, e
			}
			page, e := queryInt(q, "page", 1)
			if e != nil {
				return nil, e
			}
			perPage, e := queryInt(q, "per_page", 25)
			if e != nil {
				return nil, e
			}
			return s.Activity(ctx, a, r.PathValue("resource_type"), r.PathValue("resource_id"), page, perPage)
		})
		register("GET /api/v1/collaboration/resources/{resource_type}/{resource_id}/mentionable-users", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "search")
			if e != nil {
				return nil, e
			}
			users, e := s.MentionableUsers(ctx, a, r.PathValue("resource_type"), r.PathValue("resource_id"), q["search"])
			if e != nil {
				return nil, e
			}
			return map[string]any{"data": users}, nil
		})
		register("DELETE /api/v1/collaboration/comments/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p CommentDeleteInput
			if e := decodeRequired(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.DeleteComment(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("GET /api/v1/collaboration/notifications", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			q, e := queryValues(r, "after_id", "limit")
			if e != nil {
				return nil, e
			}
			limit, e := queryInt(q, "limit", 50)
			if e != nil {
				return nil, e
			}
			return s.Notifications(ctx, a, q["after_id"], limit)
		})
		register("POST /api/v1/collaboration/notifications/{id}/read", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p struct{}
			if e := api.Decode(r, &p); e != nil {
				return nil, e
			}
			return s.ReadNotification(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"))
		})
		register("POST /api/v1/collaboration/folders", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p FolderInput
			if e := decodeRequired(r, &p, "scope", "person_id", "team_id", "parent_id", "name"); e != nil {
				return nil, e
			}
			return s.CreateFolder(ctx, a, r.Header.Get("Idempotency-Key"), p)
		})
		register("PATCH /api/v1/collaboration/folders/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p FolderUpdate
			if e := decodeRequired(r, &p, "version", "name"); e != nil {
				return nil, e
			}
			return s.UpdateFolder(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("DELETE /api/v1/collaboration/folders/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p VersionInput
			if e := decodeRequired(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.TrashFolder(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p, false)
		})
		register("POST /api/v1/collaboration/folders/{id}/restore", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p VersionInput
			if e := decodeRequired(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.TrashFolder(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p, true)
		})
		register("POST /api/v1/collaboration", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p EntryInput
			if e := decodeRequired(r, &p, "document_id", "document_version", "scope", "person_id", "folder_id", "title", "notes"); e != nil {
				return nil, e
			}
			return s.CreateEntry(ctx, a, r.Header.Get("Idempotency-Key"), p)
		})
		register("GET /api/v1/collaboration/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			return s.Entry(ctx, a, r.PathValue("id"))
		})
		register("PATCH /api/v1/collaboration/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p EntryUpdate
			if e := decodeRequired(r, &p, "version", "folder_id", "title", "notes"); e != nil {
				return nil, e
			}
			return s.UpdateEntry(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p)
		})
		register("DELETE /api/v1/collaboration/{id}", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p VersionInput
			if e := decodeRequired(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.TrashEntry(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p, false)
		})
		register("POST /api/v1/collaboration/{id}/restore", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			var p VersionInput
			if e := decodeRequired(r, &p, "version"); e != nil {
				return nil, e
			}
			return s.TrashEntry(ctx, a, r.PathValue("id"), r.Header.Get("Idempotency-Key"), p, true)
		})
		register("GET /api/v1/collaboration/{id}/versions", func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
			if _, e := queryValues(r); e != nil {
				return nil, e
			}
			return s.Versions(ctx, a, r.PathValue("id"))
		})
		for _, route := range []struct {
			suffix  string
			preview bool
		}{
			{suffix: "preview", preview: true},
			{suffix: "download", preview: false},
		} {
			route := route
			pattern := "GET /api/v1/collaboration/{id}/" + route.suffix
			register(pattern, func(ctx context.Context, a core.Actor, r *http.Request) (any, error) {
				q, e := queryValues(r, "file_version")
				if e != nil {
					return nil, e
				}
				number := 0
				if value, ok := q["file_version"]; ok {
					number, e = strconv.Atoi(value)
					if e != nil || number < 1 {
						return nil, core.Invalid
					}
				}
				return s.Deliver(ctx, a, r.PathValue("id"), number, route.preview, objects)
			})
		}
	}}
}

func hasQuery(q map[string]string, key string) bool {
	_, ok := q[key]
	return ok
}
