package integration

import (
	"encoding/base64"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"io"
	"log/slog"
	"net/http"
	"testing"
	"time"
)

func TestQuoteApprovalHTTPStrictDTOAndDisabledWorkflow(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	r := registry.New()
	if e := r.Register(s.Module(slog.New(slog.NewTextHandler(io.Discard, nil)), true)); e != nil {
		t.Fatal(e)
	}
	h := api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	token, csrf, e := s.Core.NewSession(ctx, a.UserID, true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	cookie := &http.Cookie{Name: "bastion_dev_session", Value: token}
	for _, p := range []map[string]any{
		{"quote_version": "3", "expires_at": time.Now().Add(time.Hour).Format(time.RFC3339), "reason": nil},
		{"quote_version": 3, "expires_at": time.Now().Add(time.Hour).Format(time.RFC3339), "reason": ""},
		{"quote_version": "3", "expires_at": "not-a-date", "reason": ""},
		{"quote_version": "3", "expires_at": time.Now().Add(time.Hour).Format(time.RFC3339), "reason": "", "input_hash": "malicious"},
	} {
		status, _, _ := execute(t, h, "POST", "/api/v1/quotes/"+id+"/approvals", p, cookie, csrf, s.Core.Config.Origin)
		if status != 422 {
			t.Fatal(status, p)
		}
	}
	v := requestApproval(t, s, a, id, "http-request", 3)
	other := makeQuote(t, s, a, "http-other-quote", quoteInput())
	if _, e = s.DecideApproval(ctx, reviewer, other, v.ApprovalID, "http-other-decision", quotes.ApprovalDecision{QuoteVersion: 1, Version: 1, Decision: "approved", Reason: "Revisar"}); e != core.Missing {
		t.Fatal("approval belongs to different quote", e)
	}
	s.Approvals = nil
	r = registry.New()
	_ = r.Register(s.Module(slog.New(slog.NewTextHandler(io.Discard, nil)), true))
	h = api.New(s.Core, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	status, _, _ := execute(t, h, "GET", "/api/v1/quotes/"+id+"/approvals", nil, cookie, csrf, s.Core.Config.Origin)
	if status != 404 {
		t.Fatal("disabled route", status)
	}
}
func TestQuoteApprovalAttachmentsAvailableAndFingerprint(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	if _, e := s.Transition(ctx, a, id, "attachment-draft", quotes.TransitionInput{Status: "draft", Version: 3}); e != nil {
		t.Fatal(e)
	}
	content := "%PDF-1.7\nsynthetic"
	raw, _ := hexBytes(security.Hash(content))
	f := objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64.StdEncoding.EncodeToString(raw), Prefix: []byte(content)}}
	result, e := s.Core.CreateFile(ctx, a, "", "approval-upload", core.FileRequest{Filename: "evidence.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: security.Hash(content)}, f)
	if e != nil {
		t.Fatal(e)
	}
	doc := result.(map[string]any)["id"].(string)
	if _, e = s.LinkDocument(ctx, a, id, "approval-link", doc, 4, 1); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.FinalizeFile(ctx, a, doc, f); e != nil {
		t.Fatal(e)
	}
	for i, state := range []string{"costing", "review"} {
		if _, e = s.Transition(ctx, a, id, "attachment-"+state, quotes.TransitionInput{Status: state, Version: uint64(i + 5)}); e != nil {
			t.Fatal(e)
		}
	}
	p := quotes.ApprovalRequest{QuoteVersion: 7, Expires: time.Now().Add(time.Hour).Format(time.RFC3339Nano), Reason: "Adjunto"}
	if _, e = s.RequestApproval(ctx, a, id, "blocked-quarantine", p); e != core.Conflict {
		t.Fatal("quarantined approval", e)
	}
	if e = s.Core.ScanFile(ctx, a, doc, 1, true, "synthetic-analysis-evidence"); e != nil {
		t.Fatal(e)
	}
	// Reviewer needs both business and technical read permissions.
	if _, e = s.Core.DB.Exec(`INSERT INTO role_permissions SELECT role_id,'core.documents.read','all' FROM user_roles WHERE user_id=? LIMIT 1`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	v := requestApproval(t, s, a, id, "available-attachment", 7)
	decideApproval(t, s, reviewer, id, v, 7, "approved")
	if view := approvalView(t, s, a, id); !view.Valid {
		t.Fatal(view)
	}
	if _, e = s.Core.DB.Exec(`UPDATE document_versions SET checksum=? WHERE document_id=?`, security.Hash("changed evidence"), doc); e != nil {
		t.Fatal(e)
	}
	view := approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_changed" {
		t.Fatal(view)
	}
	// Restoring bytes alone does not restore a changed document version.
	if _, e = s.Core.DB.Exec(`UPDATE document_versions SET checksum=? WHERE document_id=?`, security.Hash(content), doc); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE documents SET version=version+1 WHERE id=?`, doc); e != nil {
		t.Fatal(e)
	}
	view = approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_changed" {
		t.Fatal(view)
	}
}

func TestQuoteApprovalTeamScopeAndSeparateRequester(t *testing.T) {
	s, creator, _, id := approvalFixture(t)
	requester := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, requester, "all")
	reviewer := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, reviewer, "team")
	if _, e := s.Core.DB.Exec(`INSERT INTO teams(id,name) VALUES(1,'Equipo aprobación sintético')`); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO team_members VALUES(1,?)`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE quotes SET team_id=1 WHERE id=?`, id); e != nil {
		t.Fatal(e)
	}
	v := requestApproval(t, s, requester, id, "different-requester", 3)
	d := quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión"}
	for _, a := range []core.Actor{creator, requester} {
		if _, e := s.DecideApproval(ctx, a, id, v.ApprovalID, "no-self-decision", d); e != core.Forbidden {
			t.Fatal(a.UserID, e)
		}
	}
	decideApproval(t, s, reviewer, id, v, 3, "approved")
	if view := approvalView(t, s, creator, id); !view.Valid {
		t.Fatal(view)
	}
	if _, e := s.Core.DB.Exec(`DELETE FROM team_members WHERE user_id=?`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	view := approvalView(t, s, creator, id)
	if view.Valid || view.InvalidReason != "approval_authority_revoked" {
		t.Fatal(view)
	}
}
