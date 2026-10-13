package integration

import (
	"context"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/collaboration"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

func TestCollaborationTasksLifecycleAndViews(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	service := &collaboration.Service{Core: s, ResourcePolicies: map[string]collaboration.ResourcePolicy{}}

	today := time.Now().In(time.FixedZone("America/Tegucigalpa", -6*60*60)).Format("2006-01-02")
	created, e := service.CreateTask(ctx, a, "task-create", collaboration.TaskInput{AssigneeID: a.UserID, Title: "Revisar expediente", Note: "Texto sintético", DueDate: today})
	if e != nil {
		t.Fatal(e)
	}
	id := created.(map[string]string)["id"]
	if id == "" || created.(map[string]string)["version"] != "1" {
		t.Fatal("task create response", created)
	}
	replay, e := service.CreateTask(ctx, a, "task-create", collaboration.TaskInput{AssigneeID: a.UserID, Title: "Revisar expediente", Note: "Texto sintético", DueDate: today})
	if e != nil || replay == nil {
		t.Fatal("task replay", replay, e)
	}
	if _, e = service.CreateTask(ctx, a, "task-create", collaboration.TaskInput{AssigneeID: a.UserID, Title: "Replay cambiado", DueDate: today}); e != core.Conflict {
		t.Fatal("changed replay accepted", e)
	}

	page, e := service.Tasks(ctx, a, collaboration.TaskFilter{View: "today", Page: 1, PerPage: 25})
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != id {
		t.Fatal("today projection", page, e)
	}
	started, e := service.ChangeTaskStatus(ctx, a, id, "task-start", collaboration.TaskStatusInput{Version: 1, Status: "in_progress"})
	if e != nil || started.(map[string]string)["version"] != "2" {
		t.Fatal("task start", started, e)
	}
	done, e := service.ChangeTaskStatus(ctx, a, id, "task-done", collaboration.TaskStatusInput{Version: 2, Status: "done"})
	if e != nil || done.(map[string]string)["version"] != "3" {
		t.Fatal("task done", done, e)
	}
	if _, e = service.ChangeTaskStatus(ctx, a, id, "task-done", collaboration.TaskStatusInput{Version: 2, Status: "done"}); e != nil {
		t.Fatal("lost done ACK", e)
	}
	if _, e = service.ChangeTaskStatus(ctx, a, id, "task-reactivate-wrong-route", collaboration.TaskStatusInput{Version: 3, Status: "open"}); e != core.Conflict {
		t.Fatal("closed task reactivated without explicit action", e)
	}
	if _, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	reopened, e := service.ReopenTask(ctx, a, id, "task-reopen", collaboration.TaskReopenInput{Version: 3})
	if e != nil || reopened.(map[string]string)["status"] != "open" || reopened.(map[string]string)["version"] != "4" {
		t.Fatal("explicit reopen", reopened, e)
	}
	if _, e = service.ReopenTask(ctx, a, id, "task-reopen", collaboration.TaskReopenInput{Version: 3}); e != nil {
		t.Fatal("lost reopen ACK", e)
	}
	s.DB.SetMaxOpenConns(1)
	bounded, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	if users, err := service.TaskAssignees(bounded, a, "", "", ""); err != nil || len(users) != 1 {
		t.Fatal("single-connection assignees", users, err)
	}
	if _, e = service.UpdateTask(ctx, a, id, "task-stale-update", collaboration.TaskUpdate{Version: 3, AssigneeID: a.UserID, Title: "Stale", DueDate: today}); e != core.Conflict {
		t.Fatal("stale task version accepted", e)
	}
	if _, e = s.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='colaboracion.view'`, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = service.ChangeTaskStatus(ctx, a, id, "task-done", collaboration.TaskStatusInput{Version: 2, Status: "done"}); e != core.Forbidden {
		t.Fatal("revoked view replay", e)
	}
	if _, e = service.ChangeTaskStatus(ctx, a, id, "task-done-after-revoke", collaboration.TaskStatusInput{Version: 4, Status: "done"}); e != core.Forbidden {
		t.Fatal("revoked view mutation", e)
	}
}

func TestCollaborationLinksReauthorizeAndPreserveSnapshot(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	version := uint64(2)
	available := true
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)
	service.ResourcePolicies["processes"] = func(_ context.Context, _ core.Querier, id string) (collaboration.Resource, error) {
		if id != "77" || !available {
			return collaboration.Resource{}, core.Missing
		}
		return collaboration.Resource{ID: id, OwnerID: a.UserID, Label: "HC-77", Version: version}, nil
	}

	content := "%PDF-1.7\nsynthetic collaboration links"
	raw, _ := hexBytes(security.Hash(content))
	store := collaborationObjects{objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64Checksum(raw), Prefix: []byte(content)}}}
	created, e := s.CreateFile(ctx, a, "", "collab-links-file", core.FileRequest{Filename: "links.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, store)
	if e != nil {
		t.Fatal(e)
	}
	doc := created.(map[string]any)["id"].(string)
	if e = s.FinalizeFile(ctx, a, doc, store); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	linked, e := service.CreateEntry(ctx, a, "collab-links-entry", collaboration.EntryInput{DocumentID: doc, DocumentVersion: 1, Scope: "company", Title: "Expediente"})
	if e != nil {
		t.Fatal(e)
	}
	entry := asCollaborationEntry(t, linked)
	set, e := service.SetLinks(ctx, a, entry.ID, "collab-links-set", collaboration.SetLinksInput{Version: entry.Version, Links: []collaboration.LinkInput{{ResourceType: "processes", ResourceID: "77", ResourceVersion: 2}}})
	if e != nil || set.(map[string]string)["version"] != "2" {
		t.Fatal("set links", set, e)
	}
	links, e := service.Links(ctx, a, entry.ID)
	if e != nil || len(links) != 1 || links[0].Label != "HC-77" || links[0].Stale {
		t.Fatal("link snapshot", links, e)
	}
	version = 3
	links, e = service.Links(ctx, a, entry.ID)
	if e != nil || len(links) != 1 || !links[0].Stale || links[0].CurrentVersion != 3 {
		t.Fatal("stale link projection", links, e)
	}
	available = false
	links, e = service.Links(ctx, a, entry.ID)
	if e != nil || len(links) != 0 {
		t.Fatal("terminal resource still disclosed", links, e)
	}
}

func TestCollaborationApprovalInboxIsWorkflowReadOnly(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	requester := customerActor(t, s, "self")
	grantCollaboration(t, s, requester, "all")
	service := &collaboration.Service{Core: s, Approvals: &workflow.Service{Core: s}}
	if _, e := s.DB.Exec(`INSERT INTO approvals(action,entity_type,entity_id,entity_version,input_hash,owner_id,team_id,view_permission,request_permission,decision_permission,revoke_permission,requested_by,status,reason,expires_at,created_at,updated_at)
		VALUES('synthetic.review','quotes',77,1,REPEAT('a',64),?,'','colaboracion.view','colaboracion.view','colaboracion.view','colaboracion.view',?,'pending','',DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 1 HOUR),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, requester.UserID, requester.UserID); e != nil {
		t.Fatal(e)
	}
	page, e := service.PendingApprovals(ctx, a, 1, 25)
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].EntityID != "77" {
		t.Fatal("approval inbox", page, e)
	}
	if requesterPage, e := service.PendingApprovals(ctx, requester, 1, 25); e != nil || requesterPage.Total != 0 {
		t.Fatal("requester saw own pending approval as decision work", requesterPage, e)
	}
	var status string
	if e = s.DB.QueryRow(`SELECT status FROM approvals WHERE entity_id=77 AND action='synthetic.review'`).Scan(&status); e != nil || status != "pending" {
		t.Fatal("read-only inbox mutated workflow", status, e)
	}
}
