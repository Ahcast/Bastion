package collaboration

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
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

func TestCommentBodyAndMentionGrantBoundary(t *testing.T) {
	if !validComment("Texto plano <script>alert(1)</script>\nsegunda línea") || validComment(" ") || validComment("texto\x00") || validComment("línea\x1b[31m") || validComment(strings.Repeat("a", 4001)) {
		t.Fatal("comment body validation boundary")
	}
	query, args := mentionGrantClause("core.documents.read", "19", "27")
	for _, clause := range []string{"rp.scope='all'", "rp.scope='self'", "rp.scope='team'", "team_members", "u.id"} {
		if !strings.Contains(query, clause) {
			t.Fatalf("mention policy clause %q missing from %s", clause, query)
		}
	}
	if len(args) != 4 || args[0] != "core.documents.read" || args[1] != "19" || args[2] != "27" || args[3] != "27" {
		t.Fatalf("unexpected mention grant arguments: %#v", args)
	}
}

func TestLinksRejectEquivalentNumericDuplicates(t *testing.T) {
	input := SetLinksInput{Version: 1, Links: []LinkInput{{ResourceType: "quotes", ResourceID: "1", ResourceVersion: 1}, {ResourceType: "quotes", ResourceID: "01", ResourceVersion: 1}}}
	if e := validateLinksInput(input); e != core.Invalid {
		t.Fatal("equivalent IDs reached SQL duplicate constraint", e)
	}
	input.Links[1].ResourceType = "invoices"
	if e := validateLinksInput(input); e != nil {
		t.Fatal("different resource types rejected", e)
	}
	input.Links[0].ResourceID = "18446744073709551615"
	input.Links[1].ResourceType = "quotes"
	input.Links[1].ResourceID = "018446744073709551615"
	if e := validateLinksInput(input); e != core.Invalid {
		t.Fatal("BIGINT equivalent duplicate", e)
	}
}
