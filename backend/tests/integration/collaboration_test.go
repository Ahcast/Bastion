package integration

import (
	"context"
	"encoding/json"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/collaboration"
)

type collaborationObjects struct{ objects }

func (f collaborationObjects) PreviewURL(context.Context, string, string, string) (string, error) {
	return "https://example.test/authorized-collaboration-preview", nil
}

func configureCollaborationPolicies(s *core.Service, service *collaboration.Service) {
	service.ResourcePolicies = map[string]collaboration.ResourcePolicy{
		"collaboration": service.CollaborationEntryResource,
	}
	s.DocumentPolicies["collaboration"] = service.DocumentPolicy
	s.DocumentBindingPolicies["collaboration"] = service.DocumentBindingPolicy
	s.DocumentVersionPolicies["collaboration"] = service.DocumentVersionPolicy
	s.DocumentTrashPolicies["collaboration"] = service.DocumentTrashPolicy
}

func grantCollaboration(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	name := "collaboration-" + a.UserID + "-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var role string
	if e := s.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM roles WHERE name=?`, name).Scan(&role); e != nil {
		t.Fatal(e)
	}
	permissions := append([]string{}, collaboration.Permissions...)
	permissions = append(permissions, "core.documents.read", "core.documents.write", "core.documents.scan")
	for _, permission := range permissions {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, role, permission, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, role); e != nil {
		t.Fatal(e)
	}
}

func asCollaborationEntry(t *testing.T, v any) collaboration.Entry {
	t.Helper()
	b, e := json.Marshal(v)
	if e != nil {
		t.Fatal(e)
	}
	var entry collaboration.Entry
	if e = json.Unmarshal(b, &entry); e != nil {
		t.Fatal(e)
	}
	return entry
}

func TestCollaborationLibraryScopeVersionQuarantineAndTrashRestore(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)

	folderValue, e := service.CreateFolder(ctx, a, "collab-folder-create", collaboration.FolderInput{Scope: "company", Name: "Manuales"})
	if e != nil {
		t.Fatal(e)
	}
	folder := folderValue.(collaboration.Folder)
	folderReplay, e := service.CreateFolder(ctx, a, "collab-folder-create", collaboration.FolderInput{Scope: "company", Name: "Manuales"})
	if e != nil || folderReplay.(json.RawMessage) == nil {
		t.Fatal("folder create replay", folderReplay, e)
	}

	content := "%PDF-1.7\nsynthetic collaboration file"
	raw, _ := hexBytes(security.Hash(content))
	store := collaborationObjects{objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64Checksum(raw), Prefix: []byte(content)}}}
	created, e := s.CreateFile(ctx, a, "", "collab-file-upload", core.FileRequest{Filename: "manual.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, store)
	if e != nil {
		t.Fatal(e)
	}
	doc := created.(map[string]any)["id"].(string)
	input := collaboration.EntryInput{DocumentID: doc, DocumentVersion: 1, Scope: "company", FolderID: folder.ID, Title: "Manual", Notes: "Carga sintética"}
	if _, e = service.CreateEntry(ctx, a, "collab-link-before-finalize", input); e != core.Conflict {
		t.Fatal("pending upload entered the library", e)
	}
	if e = s.FinalizeFile(ctx, a, doc, store); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e = service.CreateEntry(ctx, a, "collab-link-needs-reauth", input); e == nil || e.Error() != "reauth_required" {
		t.Fatal("link without recent reauthentication", e)
	}
	if _, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	linked, e := service.CreateEntry(ctx, a, "collab-file-link", input)
	if e != nil {
		t.Fatal(e)
	}
	entry := asCollaborationEntry(t, linked)
	if entry.DocumentID != doc || entry.DocumentVersion != 2 || entry.Status != "quarantined" || entry.FolderID != folder.ID {
		t.Fatal("linked entry metadata", entry)
	}
	replay, e := service.CreateEntry(ctx, a, "collab-file-link", input)
	if e != nil || asCollaborationEntry(t, replay).ID != entry.ID {
		t.Fatal("link replay", replay, e)
	}
	changed := input
	changed.Title = "Changed replay"
	if _, e = service.CreateEntry(ctx, a, "collab-file-link", changed); e != core.Conflict {
		t.Fatal("changed replay accepted", e)
	}
	if _, e = service.CreateEntry(ctx, a, "collab-second-key", input); e != core.Conflict {
		t.Fatal("document linked twice", e)
	}
	versionContent := "%PDF-1.7\nsynthetic second version"
	versionRaw, _ := hexBytes(security.Hash(versionContent))
	versionStore := objects{core.ObjectInfo{Size: int64(len(versionContent)), Checksum: base64Checksum(versionRaw), Prefix: []byte(versionContent)}}
	if _, e = s.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id JOIN roles r ON r.id=rp.role_id WHERE ur.user_id=? AND r.name=? AND rp.permission='colaboracion.upload'`, a.UserID, "collaboration-"+a.UserID+"-all"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.CreateFile(ctx, a, doc, "collab-core-version-bypass", core.FileRequest{Filename: "manual-v2.pdf", MediaType: "application/pdf", Size: int64(len(versionContent)), Checksum: security.Hash(versionContent), Version: 2}, versionStore); e != core.Missing {
		t.Fatal("generic Core.Files version bypassed colaboracion.upload", e)
	}
	if _, e = s.DB.Exec(`INSERT INTO role_permissions SELECT r.id,'colaboracion.upload','all' FROM roles r WHERE r.name=? ON DUPLICATE KEY UPDATE scope='all'`, "collaboration-"+a.UserID+"-all"); e != nil {
		t.Fatal(e)
	}

	page, e := service.List(ctx, a, collaboration.Filter{Scope: "company", FolderID: folder.ID, FolderSet: true, Search: "Manual", Page: 1, PerPage: 25})
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != entry.ID {
		t.Fatal("scoped file list", page, e)
	}
	folders, e := service.ListFolders(ctx, a, "company", "", "root", false)
	if e != nil || len(folders) != 1 || folders[0].FileCount != 1 {
		t.Fatal("folder file count", folders, e)
	}
	if e = s.ScanFile(ctx, a, doc, 1, true, "collaboration-synthetic-scan"); e != nil {
		t.Fatal(e)
	}
	if _, e = service.Deliver(ctx, a, entry.ID, 0, true, store); e != nil {
		t.Fatal("authorized preview", e)
	}

	updatedValue, e := service.UpdateEntry(ctx, a, entry.ID, "collab-file-update", collaboration.EntryUpdate{Version: 1, FolderID: folder.ID, Title: "Manual actualizado", Notes: "Nota nueva"})
	if e != nil {
		t.Fatal(e)
	}
	updated := asCollaborationEntry(t, updatedValue)
	if updated.Version != 2 || updated.Title != "Manual actualizado" || updated.DocumentVersion != 2 {
		t.Fatal("metadata update", updated)
	}
	if _, e = service.UpdateEntry(ctx, a, entry.ID, "collab-file-update", collaboration.EntryUpdate{Version: 1, FolderID: folder.ID, Title: "Manual actualizado", Notes: "Nota nueva"}); e != nil {
		t.Fatal("metadata replay", e)
	}
	if _, e = service.UpdateEntry(ctx, a, entry.ID, "collab-stale-update", collaboration.EntryUpdate{Version: 1, FolderID: folder.ID, Title: "Stale", Notes: ""}); e != core.Conflict {
		t.Fatal("stale metadata update", e)
	}
	if _, e = s.DB.Exec(`DELETE rp FROM role_permissions rp JOIN user_roles ur ON ur.role_id=rp.role_id JOIN roles r ON r.id=rp.role_id WHERE ur.user_id=? AND r.name=? AND rp.permission='colaboracion.delete'`, a.UserID, "collaboration-"+a.UserID+"-all"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.TrashFile(ctx, a, doc, "collab-core-trash-bypass", 2, false); e != core.Missing {
		t.Fatal("generic Core.Files trash bypassed collaboration.delete", e)
	}
	if _, e = s.DB.Exec(`INSERT INTO role_permissions SELECT r.id,'colaboracion.delete','all' FROM roles r WHERE r.name=? ON DUPLICATE KEY UPDATE scope='all'`, "collaboration-"+a.UserID+"-all"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}

	if _, e = service.TrashEntry(ctx, a, entry.ID, "collab-file-trash-no-reauth", collaboration.VersionInput{Version: 2}, false); e == nil || e.Error() != "reauth_required" {
		t.Fatal("trash without recent reauthentication", e)
	}
	if _, e = s.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, a.SessionHash); e != nil {
		t.Fatal(e)
	}
	trashed, e := service.TrashEntry(ctx, a, entry.ID, "collab-file-trash", collaboration.VersionInput{Version: 2}, false)
	if e != nil || trashed.(map[string]string)["version"] != "3" {
		t.Fatal("trash through Core.Files", trashed, e)
	}
	trashPage, e := service.List(ctx, a, collaboration.Filter{Scope: "company", FolderID: folder.ID, FolderSet: true, Deleted: true, Page: 1, PerPage: 25})
	if e != nil || trashPage.Total != 1 || !trashPage.Data[0].Deleted || trashPage.Data[0].DocumentVersion != 3 {
		t.Fatal("trash projection", trashPage, e)
	}
	if _, e = service.Deliver(ctx, a, entry.ID, 0, false, store); e != core.Missing {
		t.Fatal("trashed file still downloadable", e)
	}
	if _, e = service.TrashEntry(ctx, a, entry.ID, "collab-file-restore", collaboration.VersionInput{Version: 3}, true); e != nil {
		t.Fatal("restore through Core.Files", e)
	}
	if _, e = service.Entry(ctx, a, entry.ID); e != nil {
		t.Fatal("restored file inaccessible", e)
	}

	other := customerActor(t, s, "self")
	grantCollaboration(t, s, other, "self")
	if _, e = service.Entry(ctx, other, entry.ID); e != core.Missing {
		t.Fatal("foreign entry disclosed", e)
	}
	foreignList, e := service.List(ctx, other, collaboration.Filter{Scope: "company", Page: 1, PerPage: 25})
	if e != nil || foreignList.Total != 0 || len(foreignList.Data) != 0 {
		t.Fatal("foreign row count disclosed", foreignList, e)
	}
	if _, e = service.TrashFolder(ctx, a, folder.ID, "collab-nonempty-folder-trash", collaboration.VersionInput{Version: folder.Version}, false); e != core.Conflict {
		t.Fatal("nonempty folder deleted", e)
	}
	emptyValue, e := service.CreateFolder(ctx, a, "collab-empty-folder-create", collaboration.FolderInput{Scope: "company", Name: "Vacía"})
	if e != nil {
		t.Fatal(e)
	}
	empty := emptyValue.(collaboration.Folder)
	if _, e = service.TrashFolder(ctx, a, empty.ID, "collab-empty-folder-trash", collaboration.VersionInput{Version: 1}, false); e != nil {
		t.Fatal(e)
	}
	if _, e = service.TrashFolder(ctx, a, empty.ID, "collab-empty-folder-trash", collaboration.VersionInput{Version: 1}, false); e != nil {
		t.Fatal("folder trash replay", e)
	}
	if _, e = service.TrashFolder(ctx, a, empty.ID, "collab-empty-folder-restore", collaboration.VersionInput{Version: 2}, true); e != nil {
		t.Fatal("folder restore", e)
	}
	if _, e = service.TrashFolder(ctx, a, empty.ID, "collab-empty-folder-restore", collaboration.VersionInput{Version: 2}, true); e != nil {
		t.Fatal("folder restore replay", e)
	}
}

func TestCollaborationBindingRollsBackAsOneCoreTransaction(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	grantCollaboration(t, s, a, "all")
	service := &collaboration.Service{Core: s}
	configureCollaborationPolicies(s, service)
	content := "%PDF-1.7\nsynthetic rollback"
	raw, _ := hexBytes(security.Hash(content))
	store := objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64Checksum(raw), Prefix: []byte(content)}}
	created, e := s.CreateFile(ctx, a, "", "collab-rollback-file", core.FileRequest{Filename: "rollback.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, store)
	if e != nil {
		t.Fatal(e)
	}
	doc := created.(map[string]any)["id"].(string)
	if e = s.FinalizeFile(ctx, a, doc, store); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec(`CREATE TRIGGER reject_collaboration_event BEFORE INSERT ON domain_events FOR EACH ROW BEGIN IF NEW.topic='collaboration.file_linked' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic event failure'; END IF; END`); e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { _, _ = s.DB.Exec(`DROP TRIGGER IF EXISTS reject_collaboration_event`) })
	_, e = service.CreateEntry(ctx, a, "collab-rollback-link", collaboration.EntryInput{DocumentID: doc, DocumentVersion: 1, Scope: "company", Title: "Rollback"})
	if e == nil {
		t.Fatal("event failure was swallowed")
	}
	var count int
	var entity *string
	var version uint64
	if e = s.DB.QueryRow(`SELECT COUNT(*) FROM collaboration_entries`).Scan(&count); e != nil || count != 0 {
		t.Fatal("entry survived rollback", count, e)
	}
	if e = s.DB.QueryRow(`SELECT entity_type,version FROM documents WHERE id=?`, doc).Scan(&entity, &version); e != nil || entity != nil || version != 1 {
		t.Fatal("Core link survived rollback", entity, version, e)
	}
}
