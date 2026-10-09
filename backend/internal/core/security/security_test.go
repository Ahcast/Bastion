package security

import (
	"github.com/pquerna/otp/totp"
	"os"
	"testing"
	"time"
)

func TestPasswordAndCipher(t *testing.T) {
	p := "synthetic secure password 42"
	hash := Password(p)
	if !Verify(p, hash) || Verify("wrong", hash) || Verify(p, "bad") {
		t.Fatal("password verification")
	}
	if ValidPassword("123456789012345") {
		t.Fatal("common password")
	}
	key := make([]byte, 32)
	v, e := Encrypt(key, "test secret")
	if e != nil {
		t.Fatal(e)
	}
	plain, e := Decrypt(key, v)
	if e != nil || plain != "test secret" {
		t.Fatal(e)
	}
	key[0] = 1
	if _, e = Decrypt(key, v); e == nil {
		t.Fatal("wrong key accepted")
	}
}
func TestTOTPStep(t *testing.T) {
	secret, _, e := NewTOTP("synthetic@example.test")
	if e != nil {
		t.Fatal(e)
	}
	now := time.Now()
	code, e := totp.GenerateCode(secret, now)
	if e != nil {
		t.Fatal(e)
	}
	step, ok := TOTPStep(secret, code, now)
	if !ok || step != now.Unix()/30 {
		t.Fatal("factor")
	}
	if _, ok = TOTPStep(secret, "abcdef", now); ok {
		t.Fatal("invalid factor")
	}
}

func TestCuratedCompromisedPasswordDenylist(t *testing.T) {
	path := t.TempDir() + "/passwords.sha256"
	password := "known compromised test password"
	if e := os.WriteFile(path, []byte(Hash(password)+"\n"), 0600); e != nil {
		t.Fatal(e)
	}
	if e := LoadDenylist(path); e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { denyMu.Lock(); denied = map[string]struct{}{}; denyMu.Unlock() })
	if ValidPassword(password) {
		t.Fatal("compromised password accepted")
	}
	if e := os.WriteFile(path, []byte("not-a-hash\n"), 0600); e != nil {
		t.Fatal(e)
	}
	if LoadDenylist(path) == nil {
		t.Fatal("malformed corpus accepted")
	}
}
