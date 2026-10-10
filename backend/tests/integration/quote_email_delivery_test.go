package integration

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func (f *pdfObjects) ReadObject(ctx context.Context, key string, max int64) ([]byte, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	b := f.data[key]
	if b == nil {
		return nil, core.Missing
	}
	if int64(len(b)) > max {
		return nil, core.Invalid
	}
	return append([]byte{}, b...), nil
}

type quoteMailer struct {
	mu    sync.Mutex
	calls int
	last  core.Email
	err   error
}

func (m *quoteMailer) Send(ctx context.Context, settings core.SMTPSettings, password string, email core.Email) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.calls++
	m.last = email
	return m.err
}

func configureQuoteSMTP(t *testing.T, s *quotes.Service, a core.Actor) {
	t.Helper()
	if e := s.Core.SaveSetting(ctx, a, "smtp", core.Setting{Value: []byte(`{"host":"smtp.example.test","port":587,"username":"synthetic","from":"ventas@example.com","tls":"starttls"}`)}, "synthetic-password"); e != nil {
		t.Fatal(e)
	}
}

func approvedEmailIntent(t *testing.T, due time.Time) (*quotes.Service, core.Actor, string, string, *pdfObjects) {
	t.Helper()
	s, a, reviewer, id, emit := emissionFixture(t)
	f, ok := s.Store.(*pdfObjects)
	if !ok {
		t.Fatal("missing synthetic object store")
	}
	if _, e := s.Core.DB.Exec(`INSERT INTO role_permissions(role_id,permission,scope)
		SELECT ur.role_id,'core.documents.read','all' FROM user_roles ur WHERE ur.user_id=?
		ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, reviewer.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Emit(ctx, a, id, "delivery-issued", emit); e != nil {
		t.Fatal(e)
	}
	var snapshot string
	if e := s.Core.DB.QueryRow(`SELECT CAST(id AS CHAR) FROM quote_pdf_snapshots WHERE quote_id=? AND kind='issued' AND status='available'`, id).Scan(&snapshot); e != nil {
		t.Fatal(e)
	}
	v, e := s.RequestEmailAuthorization(ctx, a, id, "delivery-auth", quotes.EmailAuthorizationRequest{
		QuoteVersion:  4,
		PDFSnapshotID: snapshot,
		To:            []string{"cliente@example.com"},
		CC:            []string{"copia@example.com"},
		BCC:           []string{},
		AttachmentIDs: []string{},
		Subject:       "Cotización aprobada",
		Message:       "Adjuntamos la cotización aprobada.",
		ScheduledAt:   due.UTC().Format(time.RFC3339Nano),
		ExpiresAt:     time.Now().Add(time.Hour).UTC().Format(time.RFC3339Nano),
		Reason:        "Autorizar envío comercial",
	})
	if e != nil {
		t.Fatal(e)
	}
	r := customerResult(t, v, nil)
	if _, e = s.DecideEmailAuthorization(ctx, reviewer, id, r["id"], "delivery-decision", quotes.EmailAuthorizationDecision{Version: 1, Decision: "approved", Reason: "Revisión independiente"}); e != nil {
		t.Fatal(e)
	}
	return s, a, id, r["id"], f
}

func queueAndClaimEmail(t *testing.T, s *quotes.Service, a core.Actor, quoteID, intentID string) core.Job {
	t.Helper()
	v, e := s.QueueEmail(ctx, a, quoteID, intentID, "delivery-queue")
	if e != nil {
		t.Fatal(e)
	}
	r := customerResult(t, v, nil)
	if r["status"] != "queued" || r["job_id"] == "" || r["message_id"] == "" {
		t.Fatal(r)
	}
	if e = s.Core.PublishOutbox(ctx); e != nil {
		t.Fatal(e)
	}
	j, e := s.Core.ClaimJob(ctx)
	if e != nil {
		t.Fatal(e)
	}
	if j.ID != r["job_id"] || j.Kind != "quote.email.send" {
		t.Fatal("wrong job", j.ID, j.Kind, r)
	}
	return j
}

func TestQuoteCommercialEmailAcceptedExactlyOnce(t *testing.T) {
	s, a, quoteID, intentID, objects := approvedEmailIntent(t, time.Now().Add(-time.Second))
	configureQuoteSMTP(t, s, a)
	j := queueAndClaimEmail(t, s, a, quoteID, intentID)
	mailer := &quoteMailer{}
	h := s.CommercialEmailHandler(objects, mailer)
	if e := h(ctx, j); e != nil {
		t.Fatal(e)
	}
	if mailer.calls != 1 || len(mailer.last.To) != 1 || mailer.last.To[0] != "cliente@example.com" || len(mailer.last.CC) != 1 || len(mailer.last.Attachments) != 1 {
		t.Fatal("wrong envelope", mailer.calls, mailer.last)
	}
	if security.Hash(string(mailer.last.Attachments[0].Data)) == "" {
		t.Fatal("missing issued PDF")
	}
	var status string
	if e := s.Core.DB.QueryRow(`SELECT status FROM quote_email_intents WHERE id=?`, intentID).Scan(&status); e != nil || status != "accepted" {
		t.Fatal(status, e)
	}
	// A replay of an already accepted effect is a no-op, never a second SMTP call.
	if e := h(ctx, j); e != nil || mailer.calls != 1 {
		t.Fatal("accepted replay resent", mailer.calls, e)
	}
}

func TestQuoteCommercialEmailSMTPFailureBecomesUncertainAndNeverBlindRetries(t *testing.T) {
	s, a, quoteID, intentID, objects := approvedEmailIntent(t, time.Now().Add(-time.Second))
	configureQuoteSMTP(t, s, a)
	j := queueAndClaimEmail(t, s, a, quoteID, intentID)
	mailer := &quoteMailer{err: errors.New("synthetic smtp ambiguity")}
	h := s.CommercialEmailHandler(objects, mailer)
	if e := h(ctx, j); e != core.ErrUncertain {
		t.Fatal("smtp ambiguity lost", e)
	}
	if mailer.calls != 1 {
		t.Fatal("unexpected call count", mailer.calls)
	}
	var status string
	if e := s.Core.DB.QueryRow(`SELECT status FROM quote_email_intents WHERE id=?`, intentID).Scan(&status); e != nil || status != "uncertain" {
		t.Fatal(status, e)
	}
	mailer.err = nil
	if e := h(ctx, j); e != core.ErrUncertain || mailer.calls != 1 {
		t.Fatal("uncertain result resent", mailer.calls, e)
	}
}

func TestQuoteCommercialEmailPreparingCanResumeWithoutDuplicateExternalEffect(t *testing.T) {
	s, a, quoteID, intentID, objects := approvedEmailIntent(t, time.Now().Add(-time.Second))
	configureQuoteSMTP(t, s, a)
	j := queueAndClaimEmail(t, s, a, quoteID, intentID)
	if _, e := s.Core.DB.Exec(`UPDATE quote_email_intents SET status='preparing' WHERE id=?`, intentID); e != nil {
		t.Fatal(e)
	}
	mailer := &quoteMailer{}
	if e := s.CommercialEmailHandler(objects, mailer)(ctx, j); e != nil || mailer.calls != 1 {
		t.Fatal("safe preparation resume failed", mailer.calls, e)
	}
}

func TestQuoteEmailQueueReplayAndRevokedApprover(t *testing.T) {
	s, a, id, intent, objects := approvedEmailIntent(t, time.Now().Add(-time.Second))
	configureQuoteSMTP(t, s, a)
	j := queueAndClaimEmail(t, s, a, id, intent)
	v, e := s.QueueEmail(ctx, a, id, intent, "delivery-queue")
	if e != nil || customerResult(t, v, e)["job_id"] != j.ID {
		t.Fatal("lost ACK cannot replay", v, e)
	}
	if _, e = s.Core.DB.Exec(`DELETE ur FROM user_roles ur JOIN approvals p ON p.decided_by=ur.user_id JOIN quote_email_intents i ON i.approval_id=p.id WHERE i.id=?`, intent); e != nil {
		t.Fatal(e)
	}
	mailer := &quoteMailer{}
	if e = s.CommercialEmailHandler(objects, mailer)(ctx, j); e == nil || mailer.calls != 0 {
		t.Fatal("revoked approver allowed SMTP", e, mailer.calls)
	}
}
