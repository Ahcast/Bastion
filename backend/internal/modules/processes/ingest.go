package processes

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"sort"
	"strings"
	"time"
)

var Permissions = []string{"procesos.view", "procesos.mark", "procesos.note", "procesos.export", "procesos.admin", "crawler.runs.create", "crawler.processes.import", "crawler.runs.complete"}
var CrawlerPermissions = []string{"crawler.runs.create", "crawler.processes.import", "crawler.runs.complete"}

type Service struct{ Core *core.Service }
type RunInput struct {
	ID      string    `json:"run_id"`
	Started time.Time `json:"started_at"`
	Source  string    `json:"source"`
}
type Batch struct {
	ID        string        `json:"batch_id"`
	Schema    string        `json:"schema_version"`
	Processes []Observation `json:"processes"`
}
type ACK struct {
	BatchID   string `json:"batch_id"`
	Status    string `json:"status"`
	Received  int    `json:"received"`
	New       int    `json:"new"`
	Updated   int    `json:"updated"`
	Unchanged int    `json:"unchanged"`
	Errors    int    `json:"errors"`
}

func (s *Service) machine(ctx context.Context, q core.Querier, a core.Actor, p string) error {
	if a.Type != "service" {
		return core.Forbidden
	}
	return s.Core.Authorize(ctx, q, a, p, "", "", false)
}
func (s *Service) CreateRun(ctx context.Context, a core.Actor, k string, p RunInput) (any, error) {
	if !key(p.ID, 100) || k != p.ID || p.Source != "honducompras" || p.Started.Before(time.Date(1970, 1, 1, 0, 0, 0, 0, time.UTC)) || p.Started.After(time.Now().Add(time.Minute)) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.machine(ctx, tx, a, "crawler.runs.create"); e != nil {
			return e
		}
		_, e := tx.ExecContext(ctx, `INSERT INTO crawler_runs VALUES(?,?,?,?,UTC_TIMESTAMP(6)) ON DUPLICATE KEY UPDATE id=VALUES(id)`, p.ID, p.Source, p.Started.UTC(), a.UserID)
		if e != nil {
			return e
		}
		var source string
		var started time.Time
		if e = tx.QueryRowContext(ctx, `SELECT source,started_at FROM crawler_runs WHERE id=? FOR UPDATE`, p.ID).Scan(&source, &started); e != nil {
			return e
		}
		if source != p.Source || !started.Equal(p.Started.UTC().Truncate(time.Microsecond)) {
			return core.Conflict
		}
		out = map[string]string{"run_id": p.ID, "status": "REGISTERED"}
		return nil
	})
	return out, e
}

// Serialize by run first, then source identity in lexical order, so retry intents
// cannot partially publish state or deadlock by opposing order within a batch.
func (s *Service) delivery(ctx context.Context, a core.Actor, run, id, kind, permission string, p any, fn func(*sql.Tx) (any, error)) (any, error) {
	if !key(run, 100) || !key(id, 150) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.machine(ctx, tx, a, permission); e != nil {
			return e
		}
		var r string
		if e := tx.QueryRowContext(ctx, `SELECT id FROM crawler_runs WHERE id=? FOR UPDATE`, run).Scan(&r); e != nil {
			if e == sql.ErrNoRows {
				return core.Missing
			}
			return e
		}
		var h, ack string
		e := tx.QueryRowContext(ctx, `SELECT payload_hash,ack FROM crawler_deliveries WHERE run_id=? AND batch_id=? AND kind=?`, run, id, kind).Scan(&h, &ack)
		if e == nil {
			if h != hash(p) {
				return core.Conflict
			}
			out = json.RawMessage(ack)
			return nil
		}
		if e != sql.ErrNoRows {
			return e
		}
		// Placeholder is private to the transaction and cannot be acknowledged before commit.
		_, e = tx.ExecContext(ctx, `INSERT INTO crawler_deliveries VALUES(?,?,?,?,?,?,UTC_TIMESTAMP(6))`, run, id, kind, hash(p), string(canonical(p)), "{}")
		if e != nil {
			return e
		}
		out, e = fn(tx)
		if e != nil {
			return e
		}
		_, e = tx.ExecContext(ctx, `UPDATE crawler_deliveries SET ack=? WHERE run_id=? AND batch_id=? AND kind=?`, string(canonical(out)), run, id, kind)
		if e != nil {
			return e
		}
		return s.Core.Audit(ctx, tx, a, "crawler."+kind, "crawler_run", run, "success", map[string]string{"batch_id": id})
	})
	if e != nil {
		_ = s.Core.Audit(ctx, s.Core.DB, a, "crawler."+kind, "crawler_run", run, "rejected", map[string]string{"batch_id": id})
	}
	return out, e
}
func (s *Service) Import(ctx context.Context, a core.Actor, run, k string, p Batch) (any, error) {
	if k != p.ID || p.Schema != "1.0" || len(p.Processes) < 1 || len(p.Processes) > 100 {
		return nil, core.Invalid
	}
	seen := map[string]bool{}
	for _, o := range p.Processes {
		if e := validate(o); e != nil {
			return nil, e
		}
		if seen[o.ExternalID] {
			return nil, core.Invalid
		}
		seen[o.ExternalID] = true
	}
	return s.delivery(ctx, a, run, p.ID, "import", "crawler.processes.import", p, func(tx *sql.Tx) (any, error) {
		ack := ACK{BatchID: p.ID, Status: "IMPORTED", Received: len(p.Processes)}
		indices := make([]int, len(p.Processes))
		for i := range indices {
			indices[i] = i
		}
		sort.Slice(indices, func(i, j int) bool { return p.Processes[indices[i]].ExternalID < p.Processes[indices[j]].ExternalID })
		for _, i := range indices {
			o := p.Processes[i]
			var id, raw, clockRaw, oldHash string
			var version uint64
			// Only two distinct runs may race here; the unique identity serializes initial creation.
			result, e := tx.ExecContext(ctx, `INSERT INTO processes(source,external_id,source_url,entity,reference,description,deadline,normalized_data,field_clocks,content_hash,created_by,first_seen_at,last_seen_at,version) VALUES(?,?,?,'','','','',?,?,?, ?,?,?,0) ON DUPLICATE KEY UPDATE id=LAST_INSERT_ID(id)`, o.Source, o.ExternalID, o.URL, string(canonical(emptyState())), "{}", "", a.UserID, o.Observed.UTC(), o.Observed.UTC())
			if e != nil {
				return nil, e
			}
			n, e := result.LastInsertId()
			if e != nil {
				return nil, e
			}
			id = core.WrapID(n)
			if e = tx.QueryRowContext(ctx, `SELECT normalized_data,field_clocks,content_hash,version FROM processes WHERE id=? FOR UPDATE`, id).Scan(&raw, &clockRaw, &oldHash, &version); e != nil {
				return nil, e
			}
			old := emptyState()
			clocks := Clocks{}
			if json.Unmarshal([]byte(raw), &old) != nil || json.Unmarshal([]byte(clockRaw), &clocks) != nil {
				return nil, fmt.Errorf("invalid stored process state")
			}
			state, next, changes, e := Merge(old, clocks, o)
			if e != nil {
				return nil, e
			}
			newHash := hash(state)
			changed := newHash != oldHash
			if version == 0 {
				ack.New++
			} else if changed {
				ack.Updated++
			} else {
				ack.Unchanged++
			}
			if changed {
				version++
				_, e = tx.ExecContext(ctx, `INSERT INTO process_versions(process_id,crawler_run_id,version,content_hash,normalized_data,changes,observed_at,created_at) VALUES(?,?,?,?,?,?,?,UTC_TIMESTAMP(6))`, id, run, version, newHash, string(canonical(state)), string(canonical(changes)), o.Observed.UTC())
				if e != nil {
					return nil, e
				}
				topic := "process.updated"
				if version == 1 {
					topic = "process.created"
				}
				if e = s.Core.EmitEvent(ctx, tx, a, topic, "processes", id, version); e != nil {
					return nil, e
				}
			}
			var closing any
			if t, e := time.Parse(time.RFC3339Nano, state.Fields["deadline"]); e == nil && t.Year() >= 1970 && t.Year() <= 9999 {
				closing = t.UTC()
			}
			_, e = tx.ExecContext(ctx, `UPDATE processes SET entity=?,reference=?,description=?,deadline=?,closing_at=?,normalized_data=?,field_clocks=?,content_hash=?,first_seen_at=LEAST(first_seen_at,?),last_seen_at=GREATEST(last_seen_at,?),version=? WHERE id=?`, state.Fields["entity"], state.Fields["reference"], state.Fields["description"], state.Fields["deadline"], closing, string(canonical(state)), string(canonical(next)), newHash, o.Observed.UTC(), o.Observed.UTC(), version, id)
			if e != nil {
				return nil, e
			}
			_, e = tx.ExecContext(ctx, `INSERT INTO process_observations(process_id,run_id,batch_id,position,observed_at,raw_data) VALUES(?,?,?,?,?,?)`, id, run, p.ID, i, o.Observed.UTC(), string(canonical(o)))
			if e != nil {
				return nil, e
			}
		}
		return ack, nil
	})
}

type Complete struct {
	ID         string                     `json:"batch_id"`
	Extraction map[string]json.RawMessage `json:"extraction"`
	Import     ACK                        `json:"import"`
}

func (s *Service) Complete(ctx context.Context, a core.Actor, run, k string, p Complete) (any, error) {
	if k != p.ID+"-complete" || p.Extraction == nil {
		return nil, core.Invalid
	}
	var runID, status string
	var records int
	if json.Unmarshal(p.Extraction["run_id"], &runID) != nil || runID != run || json.Unmarshal(p.Extraction["status"], &status) != nil || (status != "SUCCESS" && status != "PARTIAL_SUCCESS" && status != "FAILED") || json.Unmarshal(p.Extraction["records"], &records) != nil || records < 0 {
		return nil, core.Invalid
	}
	return s.delivery(ctx, a, run, p.ID, "complete", "crawler.runs.complete", p, func(tx *sql.Tx) (any, error) {
		rows, e := tx.QueryContext(ctx, `SELECT batch_id,ack FROM crawler_deliveries WHERE run_id=? AND kind='import'`, run)
		if e != nil {
			return nil, e
		}
		total := ACK{}
		found := map[int]bool{}
		for rows.Next() {
			var id, raw string
			if e = rows.Scan(&id, &raw); e != nil {
				rows.Close()
				return nil, e
			}
			prefix := p.ID + "-"
			if !strings.HasPrefix(id, prefix) {
				continue
			}
			var index int
			if _, e = fmt.Sscanf(strings.TrimPrefix(id, prefix), "%d", &index); e != nil || index < 0 || id != fmt.Sprintf("%s%d", prefix, index) {
				rows.Close()
				return nil, core.Conflict
			}
			found[index] = true
			var x ACK
			if json.Unmarshal([]byte(raw), &x) != nil {
				rows.Close()
				return nil, core.Conflict
			}
			total.Received += x.Received
			total.New += x.New
			total.Updated += x.Updated
			total.Unchanged += x.Unchanged
			total.Errors += x.Errors
		}
		e = rows.Err()
		rows.Close()
		if e != nil {
			return nil, e
		}
		for i := 0; i < len(found); i++ {
			if !found[i] {
				return nil, core.Conflict
			}
		}
		if total.Received != records || p.Import.Received != total.Received || p.Import.New != total.New || p.Import.Updated != total.Updated || p.Import.Unchanged != total.Unchanged || p.Import.Errors != 0 || total.Errors != 0 {
			return nil, core.Conflict
		}
		return map[string]string{"batch_id": p.ID, "status": "COMPLETED"}, nil
	})
}
