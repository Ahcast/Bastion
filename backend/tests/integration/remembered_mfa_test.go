package integration

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"image/png"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/pquerna/otp/totp"
)

type mfaBrowser struct {
	t        *testing.T
	h        http.Handler
	ip, csrf string
	cookies  map[string]*http.Cookie
}

func newMFABrowser(t *testing.T, s *core.Service) *mfaBrowser {
	t.Helper()
	return &mfaBrowser{t: t, h: api.New(s, registry.New(), nil, slog.New(slog.NewTextHandler(io.Discard, nil))), ip: "192.0.2.10", cookies: map[string]*http.Cookie{}}
}
func (b *mfaBrowser) call(method, path string, body any, want int) map[string]any {
	b.t.Helper()
	data, _ := json.Marshal(body)
	r := httptest.NewRequest(method, "/api/v1"+path, bytes.NewReader(data))
	r.RemoteAddr = b.ip + ":5000"
	r.Header.Set("Origin", "http://localhost:4200")
	r.Header.Set("Content-Type", "application/json")
	r.Header.Set("X-CSRF-Token", b.csrf)
	// Untrusted forwarding headers cannot turn a new IP into a remembered IP.
	r.Header.Set("X-Real-IP", "192.0.2.10")
	for _, c := range b.cookies {
		r.AddCookie(c)
	}
	w := httptest.NewRecorder()
	b.h.ServeHTTP(w, r)
	if w.Code != want {
		b.t.Fatalf("%s %s: status %d, want %d", method, path, w.Code, want)
	}
	for _, c := range w.Result().Cookies() {
		if c.MaxAge < 0 {
			delete(b.cookies, c.Name)
		} else {
			b.cookies[c.Name] = c
		}
	}
	var result map[string]any
	if err := json.Unmarshal(w.Body.Bytes(), &result); err != nil {
		b.t.Fatal(err)
	}
	if csrf, ok := result["csrf_token"].(string); ok {
		b.csrf = csrf
	}
	return result
}
func (b *mfaBrowser) login(email string) bool {
	b.t.Helper()
	b.call("GET", "/auth/csrf", nil, 200)
	r := b.call("POST", "/auth/login", map[string]string{"email": email, "password": "synthetic password secure 42"}, 200)
	return r["mfa_required"].(bool)
}
func enrollMFABrowser(t *testing.T, s *core.Service) (*mfaBrowser, []any) {
	t.Helper()
	b := newMFABrowser(t, s)
	if !b.login("admin@example.test") {
		t.Fatal("first login skipped OTP")
	}
	r := b.call("POST", "/auth/mfa/enroll", map[string]string{}, 200)
	uri := r["otpauth_uri"].(string)
	qr := r["qr_data_url"].(string)
	if !strings.HasPrefix(qr, "data:image/png;base64,") {
		t.Fatal("missing local QR")
	}
	data, e := base64.StdEncoding.DecodeString(strings.TrimPrefix(qr, "data:image/png;base64,"))
	if e != nil {
		t.Fatal(e)
	}
	img, e := png.Decode(bytes.NewReader(data))
	if e != nil || img.Bounds().Dx() != 256 || img.Bounds().Dy() != 256 {
		t.Fatal("invalid QR PNG")
	}
	again := b.call("POST", "/auth/mfa/enroll", map[string]string{}, 200)
	u, e := url.Parse(uri)
	if e != nil {
		t.Fatal(e)
	}
	u2, e := url.Parse(again["otpauth_uri"].(string))
	if e != nil {
		t.Fatal(e)
	}
	if u.Query().Get("secret") != u2.Query().Get("secret") {
		t.Fatal("enrollment retry rotated secret")
	}
	code, e := totp.GenerateCode(u.Query().Get("secret"), time.Now())
	if e != nil {
		t.Fatal(e)
	}
	result := b.call("POST", "/auth/mfa/confirm", map[string]string{"code": code}, 200)
	c := b.cookies["bastion_dev_mfa"]
	if c == nil || !c.HttpOnly || c.SameSite != http.SameSiteLaxMode || c.Path != "/" || c.Domain != "" || c.MaxAge != 30*24*60*60 {
		t.Fatal("unsafe remembered cookie")
	}
	b.call("GET", "/auth/me", nil, 200)
	return b, result["recovery_codes"].([]any)
}
func expireBrowserSession(t *testing.T, s *core.Service, b *mfaBrowser) {
	t.Helper()
	if _, e := s.DB.Exec("UPDATE sessions SET last_activity=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 DAY) WHERE token_hash=?", security.Hash(b.cookies["bastion_dev_session"].Value)); e != nil {
		t.Fatal(e)
	}
}
func TestRememberedMFAExpiryLogoutAndMovedSession(t *testing.T) {
	s := fixture(t)
	b, codes := enrollMFABrowser(t, s)
	proof := b.cookies["bastion_dev_mfa"]
	// Session cookie theft alone cannot impersonate the browser, even at the same IP.
	delete(b.cookies, "bastion_dev_mfa")
	b.call("GET", "/auth/me", nil, 401)
	b.cookies[proof.Name] = proof
	b.call("GET", "/auth/me", nil, 200)
	b.call("POST", "/auth/reauth", map[string]string{"password": "incorrect"}, 401)
	b.call("POST", "/auth/reauth", map[string]string{"password": "synthetic password secure 42"}, 200)
	var expires time.Time
	if e := s.DB.QueryRow("SELECT expires_at FROM mfa_browser_verifications WHERE token_hash=?", security.Hash(proof.Value)).Scan(&expires); e != nil {
		t.Fatal(e)
	}
	expireBrowserSession(t, s, b)
	if _, e := s.CleanupExpiredAuthState(ctx); e != nil {
		t.Fatal(e)
	}
	if b.login("admin@example.test") {
		t.Fatal("idle cleanup forgot MFA")
	}
	b.call("GET", "/auth/me", nil, 200)
	var after time.Time
	if e := s.DB.QueryRow("SELECT expires_at FROM mfa_browser_verifications WHERE token_hash=?", security.Hash(proof.Value)).Scan(&after); e != nil || !after.Equal(expires) {
		t.Fatal("password login extended OTP lifetime")
	}
	if _, e := s.DB.Exec("UPDATE sessions SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND) WHERE token_hash=?", security.Hash(b.cookies["bastion_dev_session"].Value)); e != nil {
		t.Fatal(e)
	}
	if b.login("admin@example.test") {
		t.Fatal("absolute expiry forgot MFA")
	}
	b.ip = "198.51.100.8"
	b.call("GET", "/auth/me", nil, 401)
	if !b.login("admin@example.test") {
		t.Fatal("new IP skipped OTP")
	}
	b.call("POST", "/auth/mfa/challenge", map[string]string{"code": codes[0].(string)}, 200)
	b.call("GET", "/auth/me", nil, 200)
	logoutProof := b.cookies["bastion_dev_mfa"]
	expireBrowserSession(t, s, b)
	if _, e := s.CleanupExpiredAuthState(ctx); e != nil {
		t.Fatal(e)
	}
	b.call("GET", "/auth/csrf", nil, 200)
	b.call("POST", "/auth/logout", nil, 200)
	if b.cookies["bastion_dev_mfa"] != nil {
		t.Fatal("logout kept MFA cookie")
	}
	// Replaying a copy of the old cookie after logout cannot restore trust.
	b.cookies[logoutProof.Name] = logoutProof
	if !b.login("admin@example.test") {
		t.Fatal("manual logout did not revoke browser proof")
	}
	b.call("POST", "/auth/mfa/challenge", map[string]string{"code": codes[0].(string)}, 401)
}
func TestRememberedMFARequiresAccountPasswordCookieAndIP(t *testing.T) {
	s := fixture(t)
	original, _ := enrollMFABrowser(t, s)
	proof := original.cookies["bastion_dev_mfa"]
	encrypted, e := security.Encrypt(s.Config.Key, "JBSWY3DPEHPK3PXP")
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec("INSERT INTO users(email,name,password_hash,totp_secret,created_at,updated_at) VALUES('other@example.test','Otra cuenta sintética',?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))", security.Password("synthetic password secure 42"), encrypted); e != nil {
		t.Fatal(e)
	}
	for _, tc := range []struct{ name, token, ip, email string }{
		{"missing", "", "192.0.2.10", "admin@example.test"},
		{"altered", security.Token(), "192.0.2.10", "admin@example.test"},
		{"new IP", proof.Value, "198.51.100.9", "admin@example.test"},
		{"other account", proof.Value, "192.0.2.10", "other@example.test"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			b := newMFABrowser(t, s)
			b.ip = tc.ip
			if tc.token != "" {
				b.cookies[proof.Name] = &http.Cookie{Name: proof.Name, Value: tc.token}
			}
			if !b.login(tc.email) {
				t.Fatal("invalid proof skipped OTP")
			}
			b.call("GET", "/auth/me", nil, 401)
		})
	}
	b := newMFABrowser(t, s)
	b.cookies[proof.Name] = proof
	b.call("GET", "/auth/csrf", nil, 200)
	b.call("GET", "/auth/status", nil, 401)
	b.call("POST", "/auth/login", map[string]string{"email": "admin@example.test", "password": "incorrect"}, 401)
	if _, e = s.DB.Exec("UPDATE mfa_browser_verifications SET expires_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND)"); e != nil {
		t.Fatal(e)
	}
	if !b.login("admin@example.test") {
		t.Fatal("expired proof skipped OTP")
	}
	cleaned, e := s.CleanupExpiredAuthState(ctx)
	if e != nil || cleaned.BrowserVerifications != 1 {
		t.Fatal("expired proofs not cleaned", e)
	}
}

func seedRememberedUser(t *testing.T, s *core.Service) (*mfaBrowser, core.Actor) {
	t.Helper()
	a := admin(t, s)
	encrypted, e := security.Encrypt(s.Config.Key, "JBSWY3DPEHPK3PXP")
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec("INSERT INTO users(email,name,password_hash,totp_secret,created_at,updated_at) VALUES('member@example.test','Cuenta sintética',?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))", security.Password("synthetic password secure 42"), encrypted); e != nil {
		t.Fatal(e)
	}
	b := newMFABrowser(t, s)
	token := security.Token()
	if _, e = s.DB.Exec("INSERT INTO mfa_browser_verifications(token_hash,user_id,ip_hash,created_at,expires_at) VALUES(?,2,?,UTC_TIMESTAMP(6),?)", security.Hash(token), security.Hash(b.ip), time.Now().UTC().Add(core.RememberedMFALifetime)); e != nil {
		t.Fatal(e)
	}
	b.cookies["bastion_dev_mfa"] = &http.Cookie{Name: "bastion_dev_mfa", Value: token}
	if b.login("member@example.test") {
		t.Fatal("valid proof rejected")
	}
	b.call("GET", "/auth/me", nil, 200)
	return b, a
}
func TestRememberedMFARevokedWithCredentialsAndAdministration(t *testing.T) {
	for _, name := range []string{"password change", "password reset", "MFA reset", "session revocation", "role change", "deactivation"} {
		t.Run(name, func(t *testing.T) {
			s := fixture(t)
			b, a := seedRememberedUser(t, s)
			var e error
			switch name {
			case "password change":
				session, e2 := s.Session(ctx, b.cookies["bastion_dev_session"].Value)
				if e2 != nil {
					t.Fatal(e2)
				}
				actor := core.Actor{Type: "user", ID: "2", UserID: "2", SessionHash: session.Hash, Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
				e = s.ChangePassword(ctx, actor, "new synthetic password secure 42")
			case "password reset":
				r, e2 := s.CreateIdentityToken(ctx, a, "reset", "member@example.test")
				if e2 != nil {
					t.Fatal(e2)
				}
				e = s.AcceptIdentityToken(ctx, r.(map[string]any)["token"].(string), "", "new synthetic password secure 42")
			case "MFA reset":
				e = s.ResetMFA(ctx, a, "2", "synthetic identity verification")
			case "session revocation":
				e = s.RevokeSessions(ctx, a, "2")
			default:
				e = s.UpdateUser(ctx, a, "2", core.UserChange{Name: "Cuenta sintética", Active: name != "deactivation", Roles: []string{}, Version: 1})
			}
			if e != nil {
				t.Fatal(e)
			}
			var proofs, sessions int
			if e = s.DB.QueryRow("SELECT COUNT(*) FROM mfa_browser_verifications WHERE user_id=2").Scan(&proofs); e != nil {
				t.Fatal(e)
			}
			if e = s.DB.QueryRow("SELECT COUNT(*) FROM sessions WHERE user_id=2").Scan(&sessions); e != nil {
				t.Fatal(e)
			}
			if proofs != 0 || sessions != 0 {
				t.Fatal("revocation retained authentication", proofs, sessions)
			}
			b.call("GET", "/auth/me", nil, 401)
		})
	}
}

func TestRememberedMFAConcurrentLoginAndRevocation(t *testing.T) {
	for _, name := range []string{"logout", "administrative revocation"} {
		t.Run(name, func(t *testing.T) {
			s := fixture(t)
			b, adminActor := seedRememberedUser(t, s)
			old, e := s.Session(ctx, b.cookies["bastion_dev_session"].Value)
			if e != nil {
				t.Fatal(e)
			}
			token, _, e := s.NewSession(ctx, "", false, b.ip)
			if e != nil {
				t.Fatal(e)
			}
			anonymous, e := s.Session(ctx, token)
			if e != nil {
				t.Fatal(e)
			}
			actor := core.Actor{Type: "user", ID: "2", UserID: "2", Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
			start := make(chan struct{})
			errors := make(chan error, 2)
			var wg sync.WaitGroup
			wg.Add(2)
			go func() {
				defer wg.Done()
				<-start
				_, _, _, err := s.Login(ctx, anonymous, actor, "member@example.test", "synthetic password secure 42", core.BrowserProof{IP: b.ip, Token: b.cookies["bastion_dev_mfa"].Value})
				errors <- err
			}()
			go func() {
				defer wg.Done()
				<-start
				var err error
				if name == "logout" {
					err = s.Logout(ctx, old, actor, b.cookies["bastion_dev_mfa"].Value)
				} else {
					err = s.RevokeSessions(ctx, adminActor, "2")
				}
				errors <- err
			}()
			close(start)
			wg.Wait()
			close(errors)
			for err := range errors {
				if err != nil {
					t.Fatal(err)
				}
			}
			var proofs, complete int
			if e = s.DB.QueryRow("SELECT COUNT(*) FROM mfa_browser_verifications WHERE user_id=2").Scan(&proofs); e != nil {
				t.Fatal(e)
			}
			if e = s.DB.QueryRow("SELECT COUNT(*) FROM sessions WHERE user_id=2 AND mfa_complete=TRUE").Scan(&complete); e != nil {
				t.Fatal(e)
			}
			if proofs != 0 || complete != 0 {
				t.Fatal("concurrent login restored revoked verification")
			}
		})
	}
}

func TestRememberedMFALogoutRevokesJustRotatedFactor(t *testing.T) {
	s := fixture(t)
	_, _ = seedRememberedUser(t, s)
	b := newMFABrowser(t, s)
	if !b.login("member@example.test") {
		t.Fatal("new browser skipped OTP")
	}
	partial, e := s.Session(ctx, b.cookies["bastion_dev_session"].Value)
	if e != nil {
		t.Fatal(e)
	}
	recovery := "synthetic-factor-logout-recovery"
	if _, e = s.DB.Exec("INSERT INTO recovery_codes VALUES(2,?)", security.Hash(recovery)); e != nil {
		t.Fatal(e)
	}
	actor := core.Actor{Type: "user", ID: "2", UserID: "2", Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
	token, _, proof, _, e := s.CompleteMFA(ctx, partial, actor, recovery, false, b.ip)
	if e != nil {
		t.Fatal(e)
	}
	if e = s.Logout(ctx, partial, actor, ""); e != nil {
		t.Fatal(e)
	}
	if _, e = s.SessionAtIP(ctx, token, b.ip, proof); e != core.Unauthorized {
		t.Fatal("logout raced with factor and left a live session", e)
	}
	var count int
	if e = s.DB.QueryRow("SELECT COUNT(*) FROM mfa_browser_verifications WHERE token_hash=?", security.Hash(proof)).Scan(&count); e != nil || count != 0 {
		t.Fatal("rotated proof survived logout", e)
	}
}
