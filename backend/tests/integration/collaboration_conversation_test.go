package integration

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/collaboration"
)

func createConversationEntry(t *testing.T, s *core.Service, a core.Actor, service *collaboration.Service, key string) string {
	t.Helper()
	content := "%PDF-1.7\nsynthetic conversation resource"
	raw, _ := hexBytes(security.Hash(content))
	store := collaborationObjects{objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64Checksum(raw), Prefix: []byte(content)}}}
	created, e := s.CreateFile(ctx, a, "", key+"-upload", core.FileRequest{Filename: "conversation.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, store)
	if e != nil {
		t.Fatal(e)
	}
	doc := created.(map[string]any)["id"].(string)
	if e = s.FinalizeFile(ctx, a, doc, store); e != nil {
		t.Fatal(e)
	}
	value, e := service.CreateEntry(ctx, a, key+"-link", collaboration.EntryInput{DocumentID: doc, DocumentVersion: 1, Scope: "company", Title: "Conversación"})
	if e != nil {
		t.Fatal(e)
	}
	return value.(collaboration.Entry).ID
}

func TestCollaborationConversationMentionsRevocationAndActivity(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)
	entryID := createConversationEntry(t, s, a, service, "collab-conversation")

	recipient := customerActor(t, s, "self")
	grantCollaboration(t, s, recipient, "all")
	users, e := service.MentionableUsers(ctx, a, "collaboration", entryID, "Operador")
	if e != nil || len(users) != 1 || users[0].ID != recipient.UserID {
		t.Fatal("mention suggestions", users, e)
	}
	body := "Texto plano <script>alert('no ejecutar')</script>"
	created, e := service.CreateComment(ctx, a, "collaboration", entryID, "collab-comment-create", collaboration.CommentInput{Body: body, MentionUserIDs: []string{recipient.UserID}})
	if e != nil {
		t.Fatal(e)
	}
	var result map[string]string
	rawResult, _ := json.Marshal(created)
	if e = json.Unmarshal(rawResult, &result); e != nil || result["id"] == "" {
		t.Fatal("comment response", created, e)
	}
	commentID := result["id"]
	replay, e := service.CreateComment(ctx, a, "collaboration", entryID, "collab-comment-create", collaboration.CommentInput{Body: body, MentionUserIDs: []string{recipient.UserID}})
	replayJSON, _ := json.Marshal(replay)
	var replayResult map[string]string
	_ = json.Unmarshal(replayJSON, &replayResult)
	if e != nil || replayResult["id"] != commentID {
		t.Fatal("comment replay", replay, e)
	}
	if _, e = service.CreateComment(ctx, a, "collaboration", entryID, "collab-comment-create", collaboration.CommentInput{Body: "cambio", MentionUserIDs: []string{recipient.UserID}}); e != core.Conflict {
		t.Fatal("changed comment replay accepted", e)
	}

	comments, e := service.Comments(ctx, a, "collaboration", entryID, 1, 25)
	if e != nil || comments.Total != 1 || len(comments.Data) != 1 || comments.Data[0].Body != body || comments.Data[0].AuthorID != a.UserID || len(comments.Data[0].MentionUserIDs) != 1 {
		t.Fatal("comment list", comments, e)
	}
	activity, e := service.Activity(ctx, a, "collaboration", entryID, 1, 25)
	if e != nil || activity.Total != 1 || activity.Data[0].Action != "comment_added" {
		t.Fatal("activity feed", activity, e)
	}
	activityJSON, _ := json.Marshal(activity)
	if string(activityJSON) == "" || strings.Contains(string(activityJSON), body) {
		t.Fatal("activity disclosed comment body", string(activityJSON))
	}

	notifications, e := service.Notifications(ctx, recipient, "0", 25)
	if e != nil || len(notifications.Data) != 1 || notifications.Data[0].CommentID != commentID {
		t.Fatal("mention notification", notifications, e)
	}
	notificationJSON, _ := json.Marshal(notifications)
	if strings.Contains(string(notificationJSON), body) || strings.Contains(string(notificationJSON), "excerpt") {
		t.Fatal("notification included private content", string(notificationJSON))
	}
	notificationID := notifications.Data[0].ID
	if _, e = service.ReadNotification(ctx, recipient, notificationID, "collab-notification-read"); e != nil {
		t.Fatal("mark notification read", e)
	}

	if _, e = s.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id WHERE ur.user_id=? AND rp.permission='colaboracion.view'`, recipient.UserID); e != nil {
		t.Fatal(e)
	}
	comments, e = service.Comments(ctx, a, "collaboration", entryID, 1, 25)
	if e != nil || comments.Data[0].MentionUserIDs == nil || len(comments.Data[0].MentionUserIDs) != 0 {
		t.Fatal("revoked mention target remained visible", comments, e)
	}
	users, e = service.MentionableUsers(ctx, a, "collaboration", entryID, "Operador")
	if e != nil || len(users) != 0 {
		t.Fatal("revoked target remained mentionable", users, e)
	}
	notifications, e = service.Notifications(ctx, recipient, "0", 25)
	if e != nil || len(notifications.Data) != 0 || notifications.NextAfterID != notificationID {
		t.Fatal("revoked resource notification was disclosed", notifications, e)
	}
	if _, e = service.ReadNotification(ctx, recipient, notificationID, "collab-notification-read-revoked"); e != core.Missing {
		t.Fatal("revoked notification could be opened", e)
	}
	if _, e = service.Comments(ctx, recipient, "collaboration", entryID, 1, 25); e != core.Missing {
		t.Fatal("revoked resource comments could be read", e)
	}
	if _, e = s.DB.Exec(`INSERT INTO role_permissions(role_id,permission,scope) SELECT ur.role_id,'colaboracion.view','all' FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=? AND r.name=? ON DUPLICATE KEY UPDATE scope='all'`, recipient.UserID, "collaboration-"+recipient.UserID+"-all"); e != nil {
		t.Fatal(e)
	}
	if _, e = service.DeleteComment(ctx, recipient, commentID, "collab-comment-moderator-delete", collaboration.CommentDeleteInput{Version: 1}); e != nil {
		t.Fatal("moderator could not remove comment", e)
	}
	if _, e = service.DeleteComment(ctx, recipient, commentID, "collab-comment-moderator-delete", collaboration.CommentDeleteInput{Version: 1}); e != nil {
		t.Fatal("moderator delete replay", e)
	}
	comments, e = service.Comments(ctx, a, "collaboration", entryID, 1, 25)
	if e != nil || comments.Total != 0 || len(comments.Data) != 0 {
		t.Fatal("removed comment remained visible", comments, e)
	}
	activity, e = service.Activity(ctx, a, "collaboration", entryID, 1, 25)
	if e != nil || activity.Total != 2 || activity.Data[1].Action != "comment_removed" {
		t.Fatal("comment removal activity", activity, e)
	}
}

func TestCollaborationConversationRollsBackMentionSideEffects(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)
	entryID := createConversationEntry(t, s, a, service, "collab-conversation-rollback")
	recipient := customerActor(t, s, "self")
	grantCollaboration(t, s, recipient, "all")
	if _, e := s.DB.Exec(`CREATE TRIGGER reject_collaboration_notification BEFORE INSERT ON collaboration_notifications FOR EACH ROW BEGIN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic notification failure'; END`); e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { _, _ = s.DB.Exec(`DROP TRIGGER IF EXISTS reject_collaboration_notification`) })
	if _, e := service.CreateComment(ctx, a, "collaboration", entryID, "collab-comment-rollback", collaboration.CommentInput{Body: "No guardar", MentionUserIDs: []string{recipient.UserID}}); e == nil {
		t.Fatal("notification failure was swallowed")
	}
	for table := range map[string]bool{"collaboration_comments": true, "collaboration_mentions": true, "collaboration_notifications": true, "collaboration_activity": true} {
		var count int
		if e := s.DB.QueryRow(`SELECT COUNT(*) FROM ` + table).Scan(&count); e != nil || count != 0 {
			t.Fatalf("conversation partial write in %s: count=%d error=%v", table, count, e)
		}
	}
}
