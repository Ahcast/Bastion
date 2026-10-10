package processes

import (
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"testing"
	"time"
)

func observation(at time.Time, data map[string]json.RawMessage, fields ...string) Observation {
	if fields == nil {
		fields = []string{}
	}
	return Observation{Source: "honducompras", ExternalID: "official:synthetic", URL: "https://example.test/process?id=1", Observed: at, Fields: fields, Data: data, Completeness: "PARTIAL", ContentHash: hash(data), RawHash: hash("raw")}
}
func TestMergePartialLateAndDocumentEvidence(t *testing.T) {
	at := time.Now().UTC().Add(-time.Hour)
	o := observation(at, map[string]json.RawMessage{"entity": json.RawMessage(`"Entidad <sintética>"`), "description": json.RawMessage(`"A  B"`), "documents": json.RawMessage(`[{"url":"https://example.test/a.pdf","name":"A","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","download_status":"DOWNLOADED"}]`)}, "entity", "description")
	state, c, changes, e := Merge(emptyState(), Clocks{}, o)
	if e != nil || len(changes) != 3 || state.Fields["description"] != "A B" {
		t.Fatal(state, e)
	}
	late := observation(at.Add(-time.Hour), map[string]json.RawMessage{"entity": json.RawMessage(`"Vieja"`), "deadline": json.RawMessage(`"2026-11-01T10:00:00-06:00"`)}, "entity", "deadline")
	next, c, _, e := Merge(state, c, late)
	if e != nil || next.Fields["entity"] != state.Fields["entity"] || next.Fields["deadline"] == "" {
		t.Fatal(next, e)
	}
	pending := observation(at.Add(time.Minute), map[string]json.RawMessage{"documents": json.RawMessage(`[{"url":"https://example.test/a.pdf","name":"A","sha256":null,"download_status":"PENDING"}]`)})
	next, c, changes, e = Merge(next, c, pending)
	if e != nil || next.Documents[0].SHA == nil || len(changes) != 0 {
		t.Fatal(next, e)
	}
	empty := observation(at.Add(2*time.Minute), map[string]json.RawMessage{"documents": json.RawMessage(`[]`)})
	next, c, _, e = Merge(next, c, empty)
	if e != nil || len(next.Documents) != 1 {
		t.Fatal(next, e)
	}
	empty.DocumentsComplete = true
	next, c, _, e = Merge(next, c, empty)
	if e != nil || len(next.Documents) != 0 {
		t.Fatal(next, e)
	}
	next, _, _, e = Merge(next, c, o)
	if e != nil || len(next.Documents) != 0 {
		t.Fatal("late resurrection", next, e)
	}
}
func TestMergeRejectsHashUnknownFieldsAndTimestampTie(t *testing.T) {
	at := time.Now().UTC().Add(-time.Hour)
	o := observation(at, map[string]json.RawMessage{"entity": json.RawMessage(`"A"`)}, "entity")
	s, c, _, e := Merge(emptyState(), Clocks{}, o)
	if e != nil {
		t.Fatal(e)
	}
	o.Data["entity"] = json.RawMessage(`"B"`)
	if validate(o) != core.Invalid {
		t.Fatal("checksum")
	}
	o.ContentHash = hash(o.Data)
	if _, _, _, e = Merge(s, c, o); e != core.Conflict {
		t.Fatal("tie", e)
	}
	o.Data["owner_id"] = json.RawMessage(`"1"`)
	o.ContentHash = hash(o.Data)
	if validate(o) != core.Invalid {
		t.Fatal("protected field")
	}
}
