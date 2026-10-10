package processes

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestPersonalDTORequiresExplicitStringVersionAndRejectsProtectedFields(t *testing.T) {
	for _, body := range []string{`{"note":"A"}`, `{"note":"A","version":null}`, `{"note":"A","version":0}`, `{"note":"A","version":"0","created_by":"2"}`} {
		r := httptest.NewRequest("PATCH", "/api/v1/processes/1/note", strings.NewReader(body))
		r.Header.Set("Content-Type", "application/json")
		var p PersonalInput
		if e := decodePersonal(r, &p); e != core.Invalid {
			t.Fatal(body, e)
		}
	}
	r := httptest.NewRequest("PATCH", "/api/v1/processes/1/note", strings.NewReader(`{"note":"A","version":"0"}`))
	r.Header.Set("Content-Type", "application/json")
	var p PersonalInput
	if e := decodePersonal(r, &p); e != nil || p.Note == nil || *p.Note != "A" {
		t.Fatal(p, e)
	}
}
