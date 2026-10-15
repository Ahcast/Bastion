package api

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
)

func TestRememberedMFACookieProductionProtection(t *testing.T) {
	s := &Server{Service: &core.Service{Config: config.Config{Env: "production"}}}
	w := httptest.NewRecorder()
	s.mfaCookie(w, "synthetic-token")
	c := w.Result().Cookies()[0]
	if c.Name != "__Host-bastion_mfa" || !c.Secure || !c.HttpOnly || c.Domain != "" || c.Path != "/" || c.SameSite != http.SameSiteLaxMode || c.MaxAge != 30*24*60*60 {
		t.Fatal("unsafe cookie attributes")
	}
	w = httptest.NewRecorder()
	s.mfaCookie(w, "")
	if w.Result().Cookies()[0].MaxAge != -1 {
		t.Fatal("logout cookie not deleted")
	}
}
