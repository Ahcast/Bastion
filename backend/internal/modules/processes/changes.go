package processes

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"strings"
)

type Change struct {
	Kind   string          `json:"kind"`
	Field  string          `json:"field"`
	Before json.RawMessage `json:"before"`
	After  json.RawMessage `json:"after"`
}

func differences(old, next State, fields []string) []Change {
	out := []Change{}
	for _, f := range fields {
		kind := "METADATA_CHANGED"
		var before, after any
		if strings.HasPrefix(f, "document:") || strings.HasPrefix(f, "document_removed:") {
			u := strings.TrimPrefix(strings.TrimPrefix(f, "document:"), "document_removed:")
			for _, d := range old.Documents {
				if d.URL == u {
					before = d
				}
			}
			for _, d := range next.Documents {
				if d.URL == u {
					after = d
				}
			}
			kind = "DOCUMENT_CHANGED"
			if before == nil {
				kind = "DOCUMENT_ADDED"
			}
			if after == nil {
				kind = "DOCUMENT_REMOVED"
			}
			f = u
		} else {
			if v, ok := old.Fields[f]; ok {
				before = v
			}
			if v, ok := next.Fields[f]; ok {
				after = v
			}
			if f == "description" {
				kind = "DESCRIPTION_CHANGED"
			}
			if f == "deadline" {
				kind = "DEADLINE_CHANGED"
			}
		}
		out = append(out, Change{Kind: kind, Field: f, Before: canonical(before), After: canonical(after)})
	}
	return out
}
func saveChanges(ctx context.Context, tx *sql.Tx, id string, version uint64, changes []Change) error {
	if len(changes) == 0 {
		return nil
	}
	marks := make([]string, len(changes))
	args := make([]any, 0, len(changes)*6)
	for i, c := range changes {
		marks[i] = "(?,?,?,?,?,?)"
		args = append(args, id, version, c.Kind, c.Field, string(c.Before), string(c.After))
	}
	_, e := tx.ExecContext(ctx, `INSERT INTO process_changes(process_id,version,kind,field,before_data,after_data) VALUES `+strings.Join(marks, ","), args...)
	return e
}

func changes(ctx context.Context, q core.Querier, id, v string) ([]Change, error) {
	rows, e := q.QueryContext(ctx, `SELECT kind,field,before_data,after_data FROM process_changes WHERE process_id=? AND version=? ORDER BY id`, id, v)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Change{}
	for rows.Next() {
		var c Change
		if e = rows.Scan(&c.Kind, &c.Field, &c.Before, &c.After); e != nil {
			return nil, e
		}
		out = append(out, c)
	}
	return out, rows.Err()
}
