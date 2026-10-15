package integration

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	_ "github.com/go-sql-driver/mysql"
	"github.com/pquerna/otp/totp"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

var ctx = context.Background()

func fixture(t *testing.T) *core.Service {
	t.Helper()
	dsn := os.Getenv("BASTION_TEST_MYSQL_DSN")
	if dsn == "" {
		t.Skip("BASTION_TEST_MYSQL_DSN unset: MySQL integration not run")
	}
	if !strings.Contains(dsn, "/bastion_test?") {
		t.Fatal("integration requires dedicated bastion_test database")
	}
	db, e := sql.Open("mysql", dsn)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { db.Close() })
	db.SetMaxOpenConns(12)
	var version string
	if e = db.QueryRow("SELECT VERSION()").Scan(&version); e != nil {
		t.Fatal(e)
	}
	if !strings.HasPrefix(version, "5.7.") {
		t.Fatalf("MySQL 5.7 required, got %s", version)
	}
	rows, e := db.Query("SHOW TABLES")
	if e != nil {
		t.Fatal(e)
	}
	var tables []string
	for rows.Next() {
		var name string
		if e = rows.Scan(&name); e != nil {
			t.Fatal(e)
		}
		tables = append(tables, name)
	}
	rows.Close()
	conn, e := db.Conn(ctx)
	if e != nil {
		t.Fatal(e)
	}
	_, e = conn.ExecContext(ctx, "SET FOREIGN_KEY_CHECKS=0")
	if e != nil {
		t.Fatal(e)
	}
	for _, name := range tables {
		if _, e = conn.ExecContext(ctx, "DROP TABLE `"+name+"`"); e != nil {
			t.Fatal(e)
		}
	}
	_, e = conn.ExecContext(ctx, "SET FOREIGN_KEY_CHECKS=1")
	conn.Close()
	if e != nil {
		t.Fatal(e)
	}
	if e = database.Migrate(ctx, db); e != nil {
		t.Fatal(e)
	}
	if e = database.Migrate(ctx, db); e != nil {
		t.Fatal(e)
	}
	if e = database.Ready(ctx, db); e != nil {
		t.Fatal(e)
	}
	s := core.New(db, config.Config{Env: "development", Origin: "http://localhost:4200", Key: make([]byte, 32), Idle: 30 * time.Minute, Absolute: 12 * time.Hour, Reauth: 5 * time.Minute, Files: true, MCP: true, Email: true, SMTPHosts: []string{"smtp.example.test"}})
	if e = s.Bootstrap(ctx, "admin@example.test", "Administrador sintético", "synthetic password secure 42"); e != nil {
		t.Fatal(e)
	}
	return s
}
func admin(t *testing.T, s *core.Service) core.Actor {
	t.Helper()
	encrypted, e := security.Encrypt(s.Config.Key, "JBSWY3DPEHPK3PXP")
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec("UPDATE users SET totp_secret=? WHERE id=1", encrypted); e != nil {
		t.Fatal(e)
	}
	token, _, e := s.NewSession(ctx, "1", true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	if _, e = s.DB.Exec("UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?", security.Hash(token)); e != nil {
		t.Fatal(e)
	}
	return core.Actor{Type: "user", ID: "1", UserID: "1", SessionHash: security.Hash(token), Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
}
func execute(t *testing.T, h http.Handler, method, path string, body any, cookie *http.Cookie, csrf, origin string, browser ...**http.Cookie) (int, map[string]any, *http.Cookie) {
	t.Helper()
	b, _ := json.Marshal(body)
	r := httptest.NewRequest(method, path, bytes.NewReader(b))
	r.RemoteAddr = "192.0.2.10:5000"
	r.Header.Set("Content-Type", "application/json")
	r.Header.Set("Origin", origin)
	if csrf != "" {
		r.Header.Set("X-CSRF-Token", csrf)
	}
	if cookie != nil {
		r.AddCookie(cookie)
	}
	if len(browser) > 0 && *browser[0] != nil {
		r.AddCookie(*browser[0])
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	result := w.Result()
	defer result.Body.Close()
	var v map[string]any
	_ = json.NewDecoder(result.Body).Decode(&v)
	var c *http.Cookie
	for _, x := range result.Cookies() {
		if x.Name == "bastion_dev_mfa" && len(browser) > 0 {
			*browser[0] = x
		}
		if x.Name == "bastion_dev_session" {
			c = x
		}
	}
	return result.StatusCode, v, c
}
func TestAuthenticationCSRFAndRecovery(t *testing.T) {
	s := fixture(t)
	var remembered *http.Cookie
	r := registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	h := api.New(s, r, nil, slog.New(slog.NewTextHandler(io.Discard, nil)))
	status, v, c := execute(t, h, "GET", "/api/v1/auth/csrf", nil, nil, "", s.Config.Origin, &remembered)
	if status != 200 || c == nil {
		t.Fatal(status, v)
	}
	csrf := v["csrf_token"].(string)
	status, _, _ = execute(t, h, "POST", "/api/v1/auth/login", map[string]string{"email": "admin@example.test", "password": "synthetic password secure 42"}, c, "", s.Config.Origin, &remembered)
	if status != 403 {
		t.Fatal("missing CSRF", status)
	}
	status, _, _ = execute(t, h, "POST", "/api/v1/auth/login", map[string]string{}, c, csrf, "http://evil.test", &remembered)
	if status != 403 {
		t.Fatal("wrong origin", status)
	}
	status, v, newCookie := execute(t, h, "POST", "/api/v1/auth/login", map[string]string{"email": "admin@example.test", "password": "synthetic password secure 42"}, c, csrf, s.Config.Origin, &remembered)
	if status != 200 || newCookie == nil || newCookie.Value == c.Value {
		t.Fatal(status, v)
	}
	old := c
	c = newCookie
	csrf = v["csrf_token"].(string)
	status, _, _ = execute(t, h, "GET", "/api/v1/users", nil, c, "", s.Config.Origin, &remembered)
	if status != 401 {
		t.Fatal("partial session reached business", status)
	}
	status, _, _ = execute(t, h, "GET", "/api/v1/auth/status", nil, old, "", s.Config.Origin, &remembered)
	if status != 401 {
		t.Fatal("old session survived login")
	}
	status, v, _ = execute(t, h, "POST", "/api/v1/auth/mfa/enroll", map[string]string{}, c, csrf, s.Config.Origin, &remembered)
	if status != 200 {
		t.Fatal(status, v)
	}
	uri := v["otpauth_uri"].(string)
	secret := strings.Split(strings.Split(uri, "secret=")[1], "&")[0]
	code, e := totp.GenerateCode(secret, time.Now())
	if e != nil {
		t.Fatal(e)
	}
	status, v, newCookie = execute(t, h, "POST", "/api/v1/auth/mfa/confirm", map[string]string{"code": code}, c, csrf, s.Config.Origin, &remembered)
	if status != 200 {
		t.Fatal(status, v)
	}
	old = c
	c = newCookie
	csrf = v["csrf_token"].(string)
	recovery := v["recovery_codes"].([]any)[0].(string)
	status, _, _ = execute(t, h, "GET", "/api/v1/auth/me", nil, c, "", s.Config.Origin, &remembered)
	if status != 200 {
		t.Fatal(status)
	}
	status, _, _ = execute(t, h, "POST", "/api/v1/auth/reauth", map[string]string{"password": "synthetic password secure 42", "code": code}, c, csrf, s.Config.Origin, &remembered)
	if status != 401 {
		t.Fatal("TOTP replay accepted", status)
	}
	status, _, _ = execute(t, h, "POST", "/api/v1/auth/reauth", map[string]string{"password": "synthetic password secure 42", "code": recovery}, c, csrf, s.Config.Origin, &remembered)
	if status != 200 {
		t.Fatal(status)
	}
	status, _, _ = execute(t, h, "POST", "/api/v1/auth/reauth", map[string]string{"password": "synthetic password secure 42", "code": recovery}, c, csrf, s.Config.Origin, &remembered)
	if status != 401 {
		t.Fatal("recovery replay", status)
	}
	status, _, _ = execute(t, h, "GET", "/api/v1/auth/me", nil, old, "", s.Config.Origin, &remembered)
	if status != 401 {
		t.Fatal("old MFA session")
	}
	status, v, _ = execute(t, h, "GET", "/api/v1/users?per_page=101", nil, c, "", s.Config.Origin, &remembered)
	if status != 422 {
		t.Fatal(status, v)
	}
	status, _, _ = execute(t, h, "POST", "/api/v1/auth/logout", nil, c, csrf, s.Config.Origin, &remembered)
	if status != 200 {
		t.Fatal(status)
	}
	status, _, _ = execute(t, h, "GET", "/api/v1/auth/me", nil, c, "", s.Config.Origin, &remembered)
	if status != 401 {
		t.Fatal("logout ineffective")
	}
}
func TestPermissionsVersionsAndRevocation(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	r, e := s.SaveRole(ctx, a, "", core.Role{Name: "Archivos personales", Grants: []core.Grant{{Permission: "core.documents.read", Scope: "self"}}})
	if e != nil {
		t.Fatal(e)
	}
	rid := r.(map[string]string)["id"]
	invite, e := s.CreateIdentityToken(ctx, a, "invite", "person@example.test")
	if e != nil {
		t.Fatal(e)
	}
	token := invite.(map[string]any)["token"].(string)
	if e = s.AcceptIdentityToken(ctx, token, "Persona", "synthetic password person 42"); e != nil {
		t.Fatal(e)
	}
	if e = s.AcceptIdentityToken(ctx, token, "Persona", "synthetic password person 42"); e == nil {
		t.Fatal("invite reusable")
	}
	if e = s.UpdateUser(ctx, a, "2", core.UserChange{Name: "Persona", Active: true, Roles: []string{rid}, Version: 1}); e != nil {
		t.Fatal(e)
	}
	if e = s.UpdateUser(ctx, a, "2", core.UserChange{Name: "Persona", Active: true, Roles: []string{rid}, Version: 1}); e != core.Conflict {
		t.Fatal("stale version", e)
	}
	secret, _ := security.Encrypt(s.Config.Key, "JBSWY3DPEHPK3PXP")
	_, _ = s.DB.Exec("UPDATE users SET totp_secret=? WHERE id=2", secret)
	session, _, e := s.NewSession(ctx, "2", true, "192.0.2.10")
	if e != nil {
		t.Fatal(e)
	}
	person := core.Actor{Type: "user", ID: "2", UserID: "2", SessionHash: security.Hash(session)}
	if e = s.Authorize(ctx, s.DB, person, "core.users.read", "", "", false); e != core.Forbidden {
		t.Fatal("privilege leakage", e)
	}
	if e = s.Authorize(ctx, s.DB, person, "core.documents.read", "2", "", false); e != nil {
		t.Fatal(e)
	}
	if e = s.Authorize(ctx, s.DB, person, "core.documents.read", "1", "", false); e != core.Forbidden {
		t.Fatal("IDOR", e)
	}
	if e = s.UpdateUser(ctx, a, "2", core.UserChange{Name: "Persona", Active: false, Roles: []string{rid}, Version: 2}); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Session(ctx, session); e != core.Unauthorized {
		t.Fatal("deactivated session", e)
	}
}
func TestIdempotencyConcurrencyOutboxAndRollback(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	var executed atomic.Int32
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			errs <- s.Transact(ctx, func(tx *sql.Tx) error {
				_, e := s.Idempotent(ctx, tx, a, "test.operation", "synthetic-key", map[string]string{"input": "same"}, func() (any, error) {
					executed.Add(1)
					id, e := s.QueueNotification(ctx, tx, a, "Notificación sintética")
					return map[string]string{"job_id": id}, e
				})
				return e
			})
		}()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	if executed.Load() != 1 {
		t.Fatal("duplicate effect", executed.Load())
	}
	if e := s.Transact(ctx, func(tx *sql.Tx) error {
		_, e := s.Idempotent(ctx, tx, a, "test.operation", "synthetic-key", map[string]string{"input": "changed"}, func() (any, error) { return map[string]bool{"ok": true}, nil })
		return e
	}); e != core.Conflict {
		t.Fatal("different payload", e)
	}
	e := s.Transact(ctx, func(tx *sql.Tx) error {
		_, e := s.QueueNotification(ctx, tx, a, "Must roll back")
		if e != nil {
			return e
		}
		return core.Invalid
	})
	if e != core.Invalid {
		t.Fatal(e)
	}
	var n int
	_ = s.DB.QueryRow("SELECT COUNT(*) FROM outbox_events").Scan(&n)
	if n != 1 {
		t.Fatal("outbox did not roll back", n)
	}
	w := core.Worker{Service: s, Handlers: map[string]core.JobHandler{"notification": s.NotificationHandler}}
	if e = w.Run(ctx, 30); e != nil {
		t.Fatal(e)
	}
	_ = s.DB.QueryRow("SELECT COUNT(*) FROM notifications").Scan(&n)
	if n != 1 {
		t.Fatal(n)
	}
	if e = w.Run(ctx, 30); e != nil {
		t.Fatal(e)
	}
	_ = s.DB.QueryRow("SELECT COUNT(*) FROM notifications").Scan(&n)
	if n != 1 {
		t.Fatal("duplicate notification")
	}
}
func TestLeasesExpiredReclaimAndRevokedJobs(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	if e := s.Transact(ctx, func(tx *sql.Tx) error { _, e := s.QueueNotification(ctx, tx, a, "Synthetic job"); return e }); e != nil {
		t.Fatal(e)
	}
	if e := s.PublishOutbox(ctx); e != nil {
		t.Fatal(e)
	}
	first, e := s.ClaimJob(ctx)
	if e != nil {
		t.Fatal(e)
	}
	_, e = s.DB.Exec("UPDATE jobs SET lease_until=UTC_TIMESTAMP(6)-INTERVAL 1 SECOND WHERE id=?", first.ID)
	if e != nil {
		t.Fatal(e)
	}
	second, e := s.ClaimJob(ctx)
	if e != nil || first.Lease == second.Lease {
		t.Fatal("lease reclaim", e)
	}
	if e = s.FinishJob(ctx, first, "completed", "", time.Now()); e != core.Conflict {
		t.Fatal("stale lease accepted", e)
	}
	_, _ = s.DB.Exec("UPDATE users SET active=FALSE WHERE id=1")
	if e = s.JobAllowed(ctx, second); e != core.Forbidden {
		t.Fatal("revoked actor allowed", e)
	}
}

type objects struct{ info core.ObjectInfo }

func (o objects) UploadURL(context.Context, string, string, int64, string) (string, error) {
	return "https://s3.example.test/upload", nil
}
func (o objects) Info(context.Context, string) (core.ObjectInfo, error) { return o.info, nil }
func (o objects) DownloadURL(context.Context, string, string) (string, error) {
	return "https://s3.example.test/download", nil
}
func TestFilesQuarantineChecksumAndAccess(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	content := "%PDF-1.7\nsynthetic"
	hash := security.Hash(content)
	raw, _ := hexBytes(hash)
	fake := objects{core.ObjectInfo{Size: int64(len(content)), Checksum: base64.StdEncoding.EncodeToString(raw), Prefix: []byte(content)}}
	r, e := s.CreateFile(ctx, a, "", "synthetic-file", core.FileRequest{Filename: "test.pdf", MediaType: "application/pdf", Size: int64(len(content)), Checksum: hash}, fake)
	if e != nil {
		t.Fatal(e)
	}
	id := r.(map[string]any)["id"].(string)
	if _, e = s.DownloadFile(ctx, a, id, 0, fake); e != core.Forbidden {
		t.Fatal("pending available", e)
	}
	if e = s.FinalizeFile(ctx, a, id, objects{core.ObjectInfo{Size: 1, Checksum: "bad"}}); e != core.Invalid {
		t.Fatal("bad checksum", e)
	}
	if e = s.FinalizeFile(ctx, a, id, fake); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DownloadFile(ctx, a, id, 0, fake); e != core.Forbidden {
		t.Fatal("quarantine bypass", e)
	}
	if e = s.FinalizeFile(ctx, a, id, fake); e != nil {
		t.Fatal("lost verification ACK could not replay", e)
	}
	if e = s.ScanFile(ctx, a, id, 1, true, "synthetic-scan-evidence"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DownloadFile(ctx, a, id, 0, fake); e != nil {
		t.Fatal(e)
	}
	_, _ = s.DB.Exec("UPDATE documents SET entity_type='quotes',entity_id='100' WHERE id=?", id)
	if _, e = s.DownloadFile(ctx, a, id, 0, fake); e != core.Forbidden {
		t.Fatal("linked entity without owner policy", e)
	}
}
func hexBytes(s string) ([]byte, error) {
	var b []byte
	for i := 0; i < len(s); i += 2 {
		var n uint64
		_, e := fmt.Sscanf(s[i:i+2], "%x", &n)
		if e != nil {
			return nil, e
		}
		b = append(b, byte(n))
	}
	return b, nil
}

type fakeMailer struct{ calls int }

func (m *fakeMailer) Send(context.Context, core.SMTPSettings, string, core.Email) error {
	m.calls++
	return context.DeadlineExceeded
}
func TestSMTPUncertaintyNoBlindRetry(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	v, _ := json.Marshal(core.SMTPSettings{Host: "smtp.example.test", Port: 587, Username: "test", From: "test@example.test", TLS: "starttls"})
	if e := s.SaveSetting(ctx, a, "smtp", core.Setting{Value: v}, "synthetic-secret"); e != nil {
		t.Fatal(e)
	}
	result, e := s.QueueTestEmail(ctx, a, "synthetic-email")
	if e != nil {
		t.Fatal(e)
	}
	if e = s.PublishOutbox(ctx); e != nil {
		t.Fatal(e)
	}
	j, e := s.ClaimJob(ctx)
	if e != nil {
		t.Fatal(e)
	}
	m := &fakeMailer{}
	handler := s.EmailHandler(m)
	if e = handler(ctx, j); e != core.ErrUncertain {
		t.Fatal(e)
	}
	if e = handler(ctx, j); e != core.ErrUncertain || m.calls != 1 {
		t.Fatal("blind retry", e, m.calls)
	}
	emailID := result.(map[string]string)["id"]
	if e = s.ReconcileEmail(ctx, a, emailID, "failed", "synthetic-confirmation"); e != nil {
		t.Fatal(e)
	}
}
func TestMCPPrincipalRevocation(t *testing.T) {
	s := fixture(t)
	a := admin(t, s)
	v, e := s.CreateService(ctx, a, "Synthetic MCP", []string{"core.modules.read"}, time.Now().Add(time.Hour))
	if e != nil {
		t.Fatal(e)
	}
	token := v.(map[string]any)["token"].(string)
	actor, e := s.AuthenticateService(ctx, token)
	if e != nil {
		t.Fatal(e)
	}
	if e = s.Authorize(ctx, s.DB, actor, "core.modules.read", "", "", false); e != nil {
		t.Fatal(e)
	}
	if e = s.Authorize(ctx, s.DB, actor, "core.jobs.read", "", "", false); e != core.Forbidden {
		t.Fatal("scope leakage", e)
	}
	if e = s.RevokeService(ctx, a, actor.ID); e != nil {
		t.Fatal(e)
	}
	if e = s.Authorize(ctx, s.DB, actor, "core.modules.read", "", "", false); e != core.Forbidden {
		t.Fatal("revocation", e)
	}
}
func TestDirtyMigrationFailsClosed(t *testing.T) {
	s := fixture(t)
	if _, e := s.DB.Exec("UPDATE schema_migrations SET dirty=TRUE"); e != nil {
		t.Fatal(e)
	}
	if e := database.Migrate(ctx, s.DB); e == nil {
		t.Fatal("dirty migration accepted")
	}
	if e := database.Ready(ctx, s.DB); e == nil {
		t.Fatal("dirty readiness accepted")
	}
}
