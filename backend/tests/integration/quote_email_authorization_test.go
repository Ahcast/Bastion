package integration

import (
	"encoding/json"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func emailAuthorizationFixture(t *testing.T) (*quotes.Service, core.Actor, core.Actor, string, string) {
	t.Helper()
	s, a, reviewer, id, emit := emissionFixture(t)
	// Reviewing a commercial send includes reviewing the exact issued PDF and
	// attachments. The reviewer therefore needs read access to that evidence;
	// Quotes approval permissions alone are intentionally insufficient.
	if _, e := s.Core.DB.Exec(`INSERT INTO role_permissions(role_id,permission,scope)
		SELECT ur.role_id,'core.documents.read','all' FROM user_roles ur WHERE ur.user_id=?
		ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "email-auth-issued", emit); e != nil {
		t.Fatal(e)
	}
	var snapshot string
	if e := s.Core.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM quote_pdf_snapshots WHERE quote_id=? AND kind='issued' AND status='available'`, id).Scan(&snapshot); e != nil {
		t.Fatal(e)
	}
	return s, a, reviewer, id, snapshot
}

func requestEmailAuthorization(t *testing.T, s *quotes.Service, a core.Actor, id, snapshot, key string) map[string]string {
	t.Helper()
	// Truncating to the current hour makes an intentional replay with the same
	// idempotency key byte-for-byte identical while keeping the approval future-dated.
	scheduled := time.Now().UTC().Truncate(time.Hour).Add(2 * time.Hour)
	expires := scheduled.Add(time.Hour)
	v, e := s.RequestEmailAuthorization(ctx, a, id, key, quotes.EmailAuthorizationRequest{
		QuoteVersion:  4,
		PDFSnapshotID: snapshot,
		To:            []string{"cliente@example.com"},
		CC:            []string{},
		BCC:           []string{},
		AttachmentIDs: []string{},
		Subject:       "Cotización aprobada",
		Message:       "Adjuntamos la cotización aprobada.",
		ScheduledAt:   scheduled.Format(time.RFC3339Nano),
		ExpiresAt:     expires.Format(time.RFC3339Nano),
		Reason:        "Autorizar envío comercial",
	})
	if e != nil {
		t.Fatal(e)
	}
	b, _ := json.Marshal(v)
	var out map[string]string
	if e = json.Unmarshal(b, &out); e != nil {
		t.Fatal(e)
	}
	return out
}

func TestQuoteEmailAuthorizationBoundTwoPersonAndIdempotent(t *testing.T) {
	s, a, reviewer, id, snapshot := emailAuthorizationFixture(t)
	v := requestEmailAuthorization(t, s, a, id, snapshot, "email-auth-request")
	if v["status"] != "pending_approval" || v["approval_id"] == "" || v["id"] == "" {
		t.Fatal(v)
	}
	if _, e := s.DecideEmailAuthorization(ctx, a, id, v["id"], "email-auth-self", quotes.EmailAuthorizationDecision{Version: 1, Decision: "approved", Reason: "Aprobación propia"}); e != core.Forbidden {
		t.Fatal("self approval", e)
	}
	approved, e := s.DecideEmailAuthorization(ctx, reviewer, id, v["id"], "email-auth-review", quotes.EmailAuthorizationDecision{Version: 1, Decision: "approved", Reason: "Revisión humana independiente"})
	if e != nil {
		t.Fatal(e)
	}
	b, _ := json.Marshal(approved)
	var result map[string]string
	_ = json.Unmarshal(b, &result)
	if result["status"] != "approved" {
		t.Fatal(result)
	}
	var status string
	var n int
	if e = s.Core.DB.QueryRow(`SELECT status FROM quote_email_intents WHERE id=?`, v["id"]).Scan(&status); e != nil || status != "approved" {
		t.Fatal(status, e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM email_messages`).Scan(&n); e != nil || n != 0 {
		t.Fatal("authorization sent mail", n, e)
	}
	again := requestEmailAuthorization(t, s, a, id, snapshot, "email-auth-request")
	if again["id"] != v["id"] || again["approval_id"] != v["approval_id"] {
		t.Fatal("idempotency replay changed intent", v, again)
	}
}

func TestQuoteEmailAuthorizationRejectsBindingAndEvidenceChanges(t *testing.T) {
	for _, change := range []string{"recipient", "subject", "pdf"} {
		t.Run(change, func(t *testing.T) {
			s, a, reviewer, id, snapshot := emailAuthorizationFixture(t)
			v := requestEmailAuthorization(t, s, a, id, snapshot, "email-auth-change-"+change)
			var e error
			switch change {
			case "recipient":
				_, e = s.Core.DB.Exec(`UPDATE quote_email_intents SET to_json='[\"otro@example.com\"]' WHERE id=?`, v["id"])
			case "subject":
				_, e = s.Core.DB.Exec(`UPDATE quote_email_intents SET subject='Contenido modificado' WHERE id=?`, v["id"])
			case "pdf":
				_, e = s.Core.DB.Exec(`UPDATE quote_pdf_snapshots SET input_hash=REPEAT('f',64) WHERE id=?`, snapshot)
			}
			if e != nil {
				t.Fatal(e)
			}
			if _, e = s.DecideEmailAuthorization(ctx, reviewer, id, v["id"], "email-auth-decision-"+change, quotes.EmailAuthorizationDecision{Version: 1, Decision: "approved", Reason: "Intentar aprobar evidencia cambiada"}); e != core.Conflict {
				t.Fatal("changed binding approved", change, e)
			}
			var status string
			if e = s.Core.DB.QueryRow(`SELECT status FROM quote_email_intents WHERE id=?`, v["id"]).Scan(&status); e != nil || status != "pending_approval" {
				t.Fatal(status, e)
			}
		})
	}
}

func TestQuoteEmailAuthorizationRequiresIssuedCurrentPDFAndLiveGrant(t *testing.T) {
	s, a, _, id, snapshot := emailAuthorizationFixture(t)
	q, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.Save(ctx, a, id, "email-auth-edit", editQuote(q)); e != nil {
		t.Fatal(e)
	}
	if _, e = s.RequestEmailAuthorization(ctx, a, id, "email-auth-stale", quotes.EmailAuthorizationRequest{
		QuoteVersion:  4,
		PDFSnapshotID: snapshot,
		To:            []string{"cliente@example.com"},
		CC:            []string{},
		BCC:           []string{},
		AttachmentIDs: []string{},
		Subject:       "Cotización aprobada",
		Message:       "Adjuntamos la cotización aprobada.",
		ScheduledAt:   time.Now().UTC().Format(time.RFC3339Nano),
		ExpiresAt:     time.Now().Add(time.Hour).UTC().Format(time.RFC3339Nano),
		Reason:        "No debe usar PDF anterior",
	}); e != core.Conflict {
		t.Fatal("stale quote authorized", e)
	}

	s, a, _, id, snapshot = emailAuthorizationFixture(t)
	if _, e = s.Core.DB.Exec(`DELETE FROM role_permissions WHERE permission='cotizaciones.send'`); e != nil {
		t.Fatal(e)
	}
	if _, e = s.RequestEmailAuthorization(ctx, a, id, "email-auth-no-grant", quotes.EmailAuthorizationRequest{
		QuoteVersion:  4,
		PDFSnapshotID: snapshot,
		To:            []string{"cliente@example.com"},
		CC:            []string{},
		BCC:           []string{},
		AttachmentIDs: []string{},
		Subject:       "Cotización aprobada",
		Message:       "Adjuntamos la cotización aprobada.",
		ScheduledAt:   time.Now().UTC().Format(time.RFC3339Nano),
		ExpiresAt:     time.Now().Add(time.Hour).UTC().Format(time.RFC3339Nano),
		Reason:        "No debe ignorar autorización vigente",
	}); e != core.Missing {
		t.Fatal("send grant bypass", e)
	}
}

func TestQuoteEmailAuthorizationRevocationSurvivesEvidenceDrift(t *testing.T) {
	s, a, _, id, snapshot := emailAuthorizationFixture(t)
	v := requestEmailAuthorization(t, s, a, id, snapshot, "email-auth-revoke-drift")
	if _, e := s.Core.DB.Exec(`UPDATE quote_pdf_snapshots SET input_hash=REPEAT('e',64) WHERE id=?`, snapshot); e != nil {
		t.Fatal(e)
	}
	revoked, e := s.RevokeEmailAuthorizationSafe(ctx, a, id, v["id"], "email-auth-revoke-drift-action", quotes.EmailAuthorizationRevoke{Version: 1, Reason: "Cancelar autorización aunque cambie la evidencia"})
	if e != nil {
		t.Fatal(e)
	}
	b, _ := json.Marshal(revoked)
	var result map[string]string
	if e = json.Unmarshal(b, &result); e != nil || result["status"] != "revoked" {
		t.Fatal(result, e)
	}
	var status string
	if e = s.Core.DB.QueryRow(`SELECT status FROM quote_email_intents WHERE id=?`, v["id"]).Scan(&status); e != nil || status != "revoked" {
		t.Fatal(status, e)
	}
}
