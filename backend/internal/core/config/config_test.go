package config

import (
	"encoding/base64"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"os"
	"testing"
)

func TestProductionFailsClosed(t *testing.T) {
	path := t.TempDir() + "/denylist.sha256"
	if e := os.WriteFile(path, []byte(security.Hash("known compromised synthetic password")+"\n"), 0600); e != nil {
		t.Fatal(e)
	}
	t.Setenv("BASTION_PASSWORD_DENYLIST_FILE", path)
	t.Setenv("BASTION_ENCRYPTION_KEY", base64.StdEncoding.EncodeToString(make([]byte, 32)))
	t.Setenv("BASTION_MYSQL_DSN", "test:test@tcp(127.0.0.1:3306)/bastion?parseTime=true")
	t.Setenv("BASTION_FRONTEND_ORIGIN", "https://bastion.example.test")
	t.Setenv("BASTION_ENV", "production")
	t.Setenv("BASTION_DB_TUNNEL_VERIFIED", "false")
	if _, e := Load(); e == nil {
		t.Fatal("plaintext production accepted")
	}
	t.Setenv("BASTION_MYSQL_DSN", "test:test@tcp(db.example.test:3306)/bastion?parseTime=true&tls=skip-verify")
	if _, e := Load(); e == nil {
		t.Fatal("unverified TLS accepted")
	}
	t.Setenv("BASTION_MYSQL_DSN", "test:test@tcp(db.example.test:3306)/bastion?parseTime=true&tls=true")
	if _, e := Load(); e != nil {
		t.Fatal(e)
	}
}
