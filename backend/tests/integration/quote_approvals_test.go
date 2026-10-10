package integration

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
)

func approvalFixture(t *testing.T) (*quotes.Service, core.Actor, core.Actor, string) {
	t.Helper()
	s, a, _ := pdfFixture(t)
	s.Approvals = &workflow.Service{Core: s.Core}
	reviewer := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, reviewer, "all")
	id := makeQuote(t, s, a, "approval-create", quoteInput())
	for i, state := range []string{"costing", "review"} {
		if _, e := s.Transition(ctx, a, id, "approval-state-"+state, quotes.TransitionInput{Status: state, Version: uint64(i + 1)}); e != nil {
			t.Fatal(e)
		}
	}
	return s, a, reviewer, id
}
func requestApproval(t *testing.T, s *quotes.Service, a core.Actor, id, key string, version uint64) workflow.Approval {
	t.Helper()
	v, e := s.RequestApproval(ctx, a, id, "approval-"+key, quotes.ApprovalRequest{QuoteVersion: version, Expires: time.Now().Add(time.Hour).UTC().Format(time.RFC3339Nano), Reason: "Revisar emisión"})
	if e != nil {
		t.Fatal(e)
	}
	b, _ := json.Marshal(v)
	var out workflow.Approval
	if e = json.Unmarshal(b, &out); e != nil {
		t.Fatal(e)
	}
	return out
}
func decideApproval(t *testing.T, s *quotes.Service, a core.Actor, id string, v workflow.Approval, version uint64, decision string) {
	t.Helper()
	_, e := s.DecideApproval(ctx, a, id, v.ApprovalID, "decision-"+v.ApprovalID, quotes.ApprovalDecision{QuoteVersion: version, Version: v.Version, Decision: decision, Reason: "Revisión humana sintética"})
	if e != nil {
		t.Fatal(e)
	}
}
func approvalView(t *testing.T, s *quotes.Service, a core.Actor, id string) quotes.ApprovalView {
	t.Helper()
	v, e := s.ApprovalList(ctx, a, id, 1, 25)
	if e != nil {
		t.Fatal(e)
	}
	return v.(map[string]any)["data"].([]quotes.ApprovalView)[0]
}
func TestQuoteApprovalBoundDecisionAndInvalidation(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	v := requestApproval(t, s, a, id, "request", 3)
	if v.Status != "pending" || v.Binding.Version != 4 || v.Version != 1 {
		t.Fatal(v)
	}
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Status != "review" || q.Version != 3 {
		t.Fatal(q, e)
	}
	p := quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión"}
	if _, e = s.DecideApproval(ctx, a, id, v.ApprovalID, "self", p); e != core.Forbidden {
		t.Fatal("self approval", e)
	}
	decideApproval(t, s, reviewer, id, v, 3, "approved")
	view := approvalView(t, s, a, id)
	if !view.Valid || view.Status != "approved" || view.Version != 2 {
		t.Fatal(view)
	}
	if _, e = s.DecideApproval(ctx, reviewer, id, v.ApprovalID, "decision-"+v.ApprovalID, quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión humana sintética"}); e != nil {
		t.Fatal("decision replay", e)
	}
	q, e = s.Get(ctx, a, id)
	if e != nil || q.Status != "approved" || q.Version != 4 {
		t.Fatal(q, e)
	}
	// It is an internal approval, never a delivery or final PDF.
	var n int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM email_messages`).Scan(&n); e != nil {
		t.Fatal(e)
	}
	if n != 0 {
		t.Fatal("approval sent mail")
	}
	edit := editQuote(q)
	edit.Terms = "Contenido cambiado"
	if _, e = s.Save(ctx, a, id, "approval-edit", edit); e != nil {
		t.Fatal(e)
	}
	view = approvalView(t, s, a, id)
	if view.Valid || view.Status != "invalidated" {
		t.Fatal(view)
	}
	q, e = s.Get(ctx, a, id)
	if e != nil || q.Status != "draft" || q.Version != 5 {
		t.Fatal(q, e)
	}
}
func TestQuoteApprovalRejectRevokeRenewAndExpiry(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	v := requestApproval(t, s, a, id, "request", 3)
	decideApproval(t, s, reviewer, id, v, 3, "rejected")
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Status != "review" || q.Version != 4 {
		t.Fatal(q, e)
	}
	v = requestApproval(t, s, a, id, "request-next", 4)
	if _, e = s.RevokeApproval(ctx, a, id, v.ApprovalID, "approval-revoke", quotes.ApprovalRevoke{QuoteVersion: 4, Version: 1, Reason: "Corregir solicitud"}); e != nil {
		t.Fatal(e)
	}
	if v = approvalView(t, s, a, id).Approval; v.Status != "revoked" {
		t.Fatal(v)
	}
	v = requestApproval(t, s, a, id, "request-third", 5)
	if _, e = s.Core.DB.Exec(`UPDATE approvals SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND) WHERE id=?`, v.ApprovalID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DecideApproval(ctx, reviewer, id, v.ApprovalID, "expired-decision", quotes.ApprovalDecision{QuoteVersion: 5, Version: 1, Decision: "approved", Reason: "No vigente"}); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = s.RevokeApproval(ctx, a, id, v.ApprovalID, "expired-revoke", quotes.ApprovalRevoke{QuoteVersion: 5, Version: 1, Reason: "Renovar vigencia"}); e != nil {
		t.Fatal(e)
	}
	v = requestApproval(t, s, a, id, "request-fourth", 6)
	decideApproval(t, s, reviewer, id, v, 6, "approved")
	if _, e = s.Core.DB.Exec(`UPDATE approvals SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND) WHERE id=?`, v.ApprovalID); e != nil {
		t.Fatal(e)
	}
	view := approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_expired" {
		t.Fatal(view)
	}
	if _, e = s.RevokeApproval(ctx, a, id, v.ApprovalID, "revoke-approved", quotes.ApprovalRevoke{QuoteVersion: 7, Version: 2, Reason: "Vigencia vencida"}); e != nil {
		t.Fatal(e)
	}
	q, e = s.Get(ctx, a, id)
	if e != nil || q.Status != "review" || q.Version != 8 {
		t.Fatal(q, e)
	}
}
func TestQuoteApprovalCompanyTemplateAndLiveAuthority(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	v := requestApproval(t, s, a, id, "request", 3)
	// Any external content mutation after request rejects the decision.
	s.Logo = []byte("changed template fingerprint")
	if _, e := s.DecideApproval(ctx, reviewer, id, v.ApprovalID, "template-changed", quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión"}); e != core.Conflict {
		t.Fatal(e)
	}
	s.Logo = nil
	decideApproval(t, s, reviewer, id, v, 3, "approved")
	s.Stamp = []byte("changed stamp fingerprint")
	view := approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_changed" {
		t.Fatal(view)
	}
	s.Stamp = nil
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 1, Value: []byte(`{"name":"Nueva empresa sintética","tax_id":"08011900123456","address":"Dirección sintética"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	view = approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_changed" {
		t.Fatal(view)
	}
	// Restore company content without restoring its version: still invalid.
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 2, Value: []byte(`{"name":"Empresa sintética","tax_id":"08011900123456","address":"Dirección sintética"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	if view = approvalView(t, s, a, id); view.Valid {
		t.Fatal(view)
	}
	// New request/decision binds the new company revision.
	if _, e := s.RevokeApproval(ctx, a, id, v.ApprovalID, "approval-revoke", quotes.ApprovalRevoke{QuoteVersion: 4, Version: 2, Reason: "Empresa actualizada"}); e != nil {
		t.Fatal(e)
	}
	v = requestApproval(t, s, a, id, "request-new-company", 5)
	decideApproval(t, s, reviewer, id, v, 5, "approved")
	if _, e := s.Core.DB.Exec(`UPDATE users SET active=FALSE WHERE id=?`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	view = approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_authority_revoked" {
		t.Fatal(view)
	}
	if _, e := s.Core.DB.Exec(`UPDATE users SET active=TRUE,totp_secret=NULL WHERE id=?`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	view = approvalView(t, s, a, id)
	if view.Valid || view.InvalidReason != "approval_authority_revoked" {
		t.Fatal(view)
	}
}
func TestQuoteApprovalConcurrentRequestsDecisionsAndAuditRollback(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	p := quotes.ApprovalRequest{QuoteVersion: 3, Expires: time.Now().Add(time.Hour).Format(time.RFC3339Nano), Reason: "Revisar"}
	var wg sync.WaitGroup
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, e := s.RequestApproval(ctx, a, id, "same-request", p); e != nil {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	v := approvalView(t, s, a, id).Approval
	var n int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM approvals`).Scan(&n); e != nil || n != 1 {
		t.Fatal(n, e)
	}
	if _, e := s.Core.DB.Exec(`CREATE TRIGGER reject_approval_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic audit failure'`); e != nil {
		t.Fatal(e)
	}
	d := quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión"}
	if _, e := s.DecideApproval(ctx, reviewer, id, v.ApprovalID, "rollback-decision", d); e == nil {
		t.Fatal("ignored audit failure")
	}
	q, e := s.Get(ctx, a, id)
	if e != nil || q.Version != 3 || q.Status != "review" {
		t.Fatal(q, e)
	}
	if view := approvalView(t, s, a, id); view.Status != "pending" || view.Version != 1 {
		t.Fatal(view)
	}
	if _, e = s.Core.DB.Exec(`DROP TRIGGER reject_approval_audit`); e != nil {
		t.Fatal(e)
	}
	var winners atomic.Int32
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			_, e := s.DecideApproval(ctx, reviewer, id, v.ApprovalID, fmt.Sprintf("decide-%d", i), d)
			if e == nil {
				winners.Add(1)
			} else if e != core.Conflict {
				t.Error(e)
			}
		}(i)
	}
	wg.Wait()
	if winners.Load() != 1 {
		t.Fatal(winners.Load())
	}
	if view := approvalView(t, s, a, id); !view.Valid || view.Version != 2 {
		t.Fatal(view)
	}
	for table, want := range map[string]int{"approval_events": 2, "quote_versions": 4} {
		if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM ` + table).Scan(&n); e != nil || n != want {
			t.Fatal(table, n, e)
		}
	}
}
func TestQuoteApprovalScopesReauthAndLiveRevocation(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	v := requestApproval(t, s, a, id, "request", 3)
	d := quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión"}
	stranger := customerActor(t, s.Core, "self")
	grantQuote(t, s.Core, stranger, "self")
	if _, e := s.ApprovalList(ctx, stranger, id, 1, 25); e != core.Missing {
		t.Fatal("IDOR", e)
	}
	if _, e := s.DecideApproval(ctx, stranger, id, v.ApprovalID, "approval-idor", d); e != core.Missing {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=NULL WHERE token_hash=?`, reviewer.SessionHash); e != nil {
		t.Fatal(e)
	}
	if _, e := s.DecideApproval(ctx, reviewer, id, v.ApprovalID, "approval-reauth", d); e == nil || e.Error() != "reauth_required" {
		t.Fatal(e)
	}
	if _, e := s.Core.DB.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, reviewer.SessionHash); e != nil {
		t.Fatal(e)
	}
	decideApproval(t, s, reviewer, id, v, 3, "approved")
	if _, e := s.Core.DB.Exec(`DELETE FROM user_roles WHERE user_id=?`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.DecideApproval(ctx, reviewer, id, v.ApprovalID, "decision-"+v.ApprovalID, quotes.ApprovalDecision{QuoteVersion: 3, Version: 1, Decision: "approved", Reason: "Revisión humana sintética"}); e != core.Forbidden {
		t.Fatal("cached replay bypass", e)
	}
	if view := approvalView(t, s, a, id); view.Valid || view.InvalidReason != "approval_authority_revoked" {
		t.Fatal(view)
	}
	machine := a
	machine.Type = "service"
	if _, e := s.RequestApproval(ctx, machine, id, "machine", quotes.ApprovalRequest{QuoteVersion: 4}); e != core.Forbidden {
		t.Fatal(e)
	}

}
func TestQuoteApprovalConsumeIsAtomicAndSingleUse(t *testing.T) {
	s, a, reviewer, id := approvalFixture(t)
	v := requestApproval(t, s, a, id, "request", 3)
	decideApproval(t, s, reviewer, id, v, 3, "approved")
	fail := fmt.Errorf("synthetic business failure")
	consume := func(commit bool) error {
		return s.Core.Transact(ctx, func(tx *sql.Tx) error {
			b, e := s.VerifyApprovalInTransaction(ctx, tx, a, id, v.ApprovalID)
			if e != nil {
				return e
			}
			if e = s.Approvals.Consume(ctx, tx, a, v.ApprovalID, b, "cotizaciones.pdf"); e != nil {
				return e
			}
			if !commit {
				return fail
			}
			return nil
		})
	}
	if e := consume(false); e != fail {
		t.Fatal(e)
	}
	if view := approvalView(t, s, a, id); !view.Valid || view.Version != 2 {
		t.Fatal("rolled back consume", view)
	}
	if e := consume(true); e != nil {
		t.Fatal(e)
	}
	if e := consume(true); e == nil || e.Error() != "approval_not_active" {
		t.Fatal("double consume", e)
	}
	if view := approvalView(t, s, a, id); view.Status != "consumed" || view.Version != 3 {
		t.Fatal(view)
	}
}
func TestQuoteApprovalCommittedRequestReplayAfterDeadline(t *testing.T) {
	s, a, _, id := approvalFixture(t)
	deadline := time.Now().Add(time.Second)
	p := quotes.ApprovalRequest{QuoteVersion: 3, Expires: deadline.Format(time.RFC3339Nano), Reason: "Vigencia breve"}
	if _, e := s.RequestApproval(ctx, a, id, "deadline-replay", p); e != nil {
		t.Fatal(e)
	}
	time.Sleep(time.Until(deadline) + time.Millisecond)
	if _, e := s.RequestApproval(ctx, a, id, "deadline-replay", p); e != nil {
		t.Fatal("lost ACK cannot recover", e)
	}
	if _, e := s.RequestApproval(ctx, a, id, "new-expired", p); e != core.Invalid {
		t.Fatal(e)
	}
}
