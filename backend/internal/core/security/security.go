package security

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"github.com/pquerna/otp"
	"github.com/pquerna/otp/totp"
	"golang.org/x/crypto/argon2"
	"image/png"
	"net/url"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"
)

func Token() string {
	b := make([]byte, 32)
	if _, e := rand.Read(b); e != nil {
		panic(e)
	}
	return base64.RawURLEncoding.EncodeToString(b)
}
func ID() string {
	b := make([]byte, 16)
	if _, e := rand.Read(b); e != nil {
		panic(e)
	}
	return hex.EncodeToString(b)
}
func Hash(s string) string   { h := sha256.Sum256([]byte(s)); return hex.EncodeToString(h[:]) }
func Equal(a, b string) bool { return subtle.ConstantTimeCompare([]byte(a), []byte(b)) == 1 }
func ValidPassword(p string) bool {
	if isDenied(p) {
		return false
	}
	if utf8.RuneCountInString(p) < 15 || len(p) > 512 {
		return false
	}
	for _, bad := range []string{"passwordpassword", "123456789012345", "qwertyuiopasdfgh", "bastionbastionbastion"} {
		if strings.ToLower(p) == bad {
			return false
		}
	}
	return true
}

var passwordSlots = make(chan struct{}, 2)

func Password(p string) string {
	passwordSlots <- struct{}{}
	defer func() { <-passwordSlots }()
	s := make([]byte, 16)
	if _, e := rand.Read(s); e != nil {
		panic(e)
	}
	h := argon2.IDKey([]byte(p), s, 3, 32*1024, 1, 32)
	return fmt.Sprintf("$argon2id$v=19$m=32768,t=3,p=1$%s$%s", base64.RawStdEncoding.EncodeToString(s), base64.RawStdEncoding.EncodeToString(h))
}
func Verify(p, encoded string) bool {
	passwordSlots <- struct{}{}
	defer func() { <-passwordSlots }()
	parts := strings.Split(encoded, "$")
	if len(parts) != 6 || parts[1] != "argon2id" || parts[2] != "v=19" || parts[3] != "m=32768,t=3,p=1" || len(p) > 512 {
		return false
	}
	s, e := base64.RawStdEncoding.DecodeString(parts[4])
	if e != nil || len(s) != 16 {
		return false
	}
	h, e := base64.RawStdEncoding.DecodeString(parts[5])
	if e != nil || len(h) != 32 {
		return false
	}
	actual := argon2.IDKey([]byte(p), s, 3, 32*1024, 1, 32)
	return subtle.ConstantTimeCompare(actual, h) == 1
}
func Encrypt(key []byte, plain string) (string, error) {
	block, e := aes.NewCipher(key)
	if e != nil {
		return "", e
	}
	a, e := cipher.NewGCM(block)
	if e != nil {
		return "", e
	}
	n := make([]byte, a.NonceSize())
	if _, e = rand.Read(n); e != nil {
		return "", e
	}
	return base64.RawStdEncoding.EncodeToString(a.Seal(n, n, []byte(plain), nil)), nil
}
func Decrypt(key []byte, value string) (string, error) {
	b, e := base64.RawStdEncoding.DecodeString(value)
	if e != nil {
		return "", e
	}
	block, e := aes.NewCipher(key)
	if e != nil {
		return "", e
	}
	a, e := cipher.NewGCM(block)
	if e != nil {
		return "", e
	}
	if len(b) < a.NonceSize() {
		return "", fmt.Errorf("invalid ciphertext")
	}
	p, e := a.Open(nil, b[:a.NonceSize()], b[a.NonceSize():], nil)
	return string(p), e
}
func NewTOTP(email string) (string, string, error) {
	k, e := totp.Generate(totp.GenerateOpts{Issuer: "Bastion", AccountName: email, SecretSize: 20})
	if e != nil {
		return "", "", e
	}
	return k.Secret(), k.URL(), nil
}

// Return matched time step so a code cannot be reused across sessions or recovery operations.
func TOTPStep(secret, code string, now time.Time) (int64, bool) {
	if len(code) != 6 {
		return 0, false
	}
	if _, e := strconv.Atoi(code); e != nil {
		return 0, false
	}
	step := now.Unix() / 30
	for _, offset := range []int64{0, -1, 1} {
		t := time.Unix((step+offset)*30, 0)
		ok, e := totp.ValidateCustom(code, secret, t, totp.ValidateOpts{Period: 30, Skew: 0, Digits: otp.DigitsSix, Algorithm: otp.AlgorithmSHA1})
		if e == nil && ok {
			return step + offset, true
		}
	}
	return 0, false
}

// TOTPURI reconstructs pending enrollment without rotating its secret.
func TOTPURI(email, secret string) string {
	u := url.URL{Scheme: "otpauth", Host: "totp", Path: "/Bastion:" + email}
	q := url.Values{"issuer": {"Bastion"}, "secret": {secret}, "period": {"30"}, "algorithm": {"SHA1"}, "digits": {"6"}}
	u.RawQuery = q.Encode()
	return u.String()
}

// TOTPQR renders entirely in-process; enrollment secrets never leave Bastion.
func TOTPQR(uri string) (string, error) {
	key, err := otp.NewKeyFromURL(uri)
	if err != nil {
		return "", err
	}
	img, err := key.Image(256, 256)
	if err != nil {
		return "", err
	}
	var data bytes.Buffer
	if err := png.Encode(&data, img); err != nil {
		return "", err
	}
	return "data:image/png;base64," + base64.StdEncoding.EncodeToString(data.Bytes()), nil
}
