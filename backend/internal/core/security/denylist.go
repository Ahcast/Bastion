package security

import (
	"bufio"
	"encoding/hex"
	"fmt"
	"os"
	"strings"
	"sync"
)

var denyMu sync.RWMutex
var denied = map[string]struct{}{}

// LoadDenylist loads locally curated SHA256 hashes without storing plaintext passwords in app config.
// A bounded corpus avoids unbounded memory on the 2 GB target. Larger breach checks require a separate policy decision.
func LoadDenylist(path string) error {
	file, e := os.Open(path)
	if e != nil {
		return fmt.Errorf("password denylist unavailable")
	}
	defer file.Close()
	next := map[string]struct{}{}
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.ToLower(strings.TrimSpace(scanner.Text()))
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		b, e := hex.DecodeString(line)
		if e != nil || len(b) != 32 {
			return fmt.Errorf("invalid password denylist hash")
		}
		next[line] = struct{}{}
		if len(next) > 100000 {
			return fmt.Errorf("password denylist too large")
		}
	}
	if e = scanner.Err(); e != nil {
		return e
	}
	if len(next) == 0 {
		return fmt.Errorf("empty password denylist")
	}
	denyMu.Lock()
	denied = next
	denyMu.Unlock()
	return nil
}
func isDenied(password string) bool {
	denyMu.RLock()
	defer denyMu.RUnlock()
	_, ok := denied[Hash(password)]
	return ok
}
