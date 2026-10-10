package integration

import (
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
)

func TestQuoteSentOutcomesRevisionAndReplay(t *testing.T) {
	for _, outcome := range []string{"accepted", "rejected", "expired"} {
		t.Run(outcome, func(t *testing.T) {
			s, a, id, intent, objects := approvedEmailIntent(t, time.Now().Add(-time.Second))
			configureQuoteSMTP(t, s, a)
			job := queueAndClaimEmail(t, s, a, id, intent)
			mailer := &quoteMailer{}
			h := s.CommercialEmailHandler(objects, mailer)
			if e := h(ctx, job); e != nil {
				t.Fatal(e)
			}
			q, e := s.Get(ctx, a, id)
			if e != nil || q.Status != "sent" || q.Version != 5 {
				t.Fatal(q, e)
			}
			if _, e = s.Save(ctx, a, id, "outcome-edit-denied", editQuote(q)); e != core.Conflict {
				t.Fatal("sent edited without reopening", e)
			}
			p := quotes.TransitionInput{Status: outcome, Version: 5, Reason: "Respuesta comercial documentada"}
			if outcome == "expired" {
				if _, e = s.Core.DB.Exec(`UPDATE quotes SET date='9999-01-01' WHERE id=?`, id); e != nil {
					t.Fatal(e)
				}
				if _, e = s.Transition(ctx, a, id, "outcome-too-early", p); e != core.Conflict {
					t.Fatal("premature expiry", e)
				}
				if _, e = s.Core.DB.Exec(`UPDATE quotes SET date='2000-01-01' WHERE id=?`, id); e != nil {
					t.Fatal(e)
				}
			}
			if _, e = s.Transition(ctx, a, id, "outcome-confirmed", p); e != nil {
				t.Fatal(e)
			}
			if _, e = s.Transition(ctx, a, id, "outcome-confirmed", p); e != nil {
				t.Fatal("replay", e)
			}
			q, e = s.Get(ctx, a, id)
			if e != nil || q.Status != outcome || q.Version != 6 {
				t.Fatal(q, e)
			}
			if _, e = s.Transition(ctx, a, id, "outcome-reopen-no-reason", quotes.TransitionInput{Status: "draft", Version: 6}); e != core.Invalid {
				t.Fatal("missing reason", e)
			}
			if _, e = s.Transition(ctx, a, id, "outcome-reopen", quotes.TransitionInput{Status: "draft", Version: 6, Reason: "Negociar nueva revisión"}); e != nil {
				t.Fatal(e)
			}
			if e = h(ctx, job); e != nil || mailer.calls != 1 {
				t.Fatal("historical delivery resent", e, mailer.calls)
			}
			q, e = s.Get(ctx, a, id)
			if e != nil || q.Status != "draft" || q.Version != 7 {
				t.Fatal(q, e)
			}
		})
	}
}
