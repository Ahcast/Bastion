package collaboration

import (
	"strings"
	"testing"
)

func TestScopeQueryAndInputBounds(t *testing.T) {
	query, args := permissionSQL("17", "colaboracion.view", "CAST(e.uploaded_by AS CHAR)", "CAST(e.team_id AS CHAR)")
	for _, condition := range []string{"rpx.scope='all'", "rpx.scope='self'", "rpx.scope='team'", "team_members"} {
		if !strings.Contains(query, condition) {
			t.Fatalf("scope clause %q missing from %s", condition, query)
		}
	}
	if len(args) != 4 || args[0] != "17" || args[1] != "colaboracion.view" {
		t.Fatalf("unexpected scope arguments: %#v", args)
	}
	if !validName("  Documento  ", 120) || validName("\nDocumento", 120) || validName(strings.Repeat("é", 121), 120) {
		t.Fatal("name validation did not enforce trimming, controls and rune limit")
	}
	if !pageBounds(1, 1) || !pageBounds(100000, 100) || pageBounds(0, 1) || pageBounds(1, 101) || pageBounds(100001, 1) {
		t.Fatal("invalid pagination bounds")
	}
}
