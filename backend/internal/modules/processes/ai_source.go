package processes

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"strings"
)

// AIInput is a read-only owner snapshot contract, independent of AI runtime.
type AIInput struct {
	ID, Owner, Team, Hash string
	Version               uint64
	Data                  json.RawMessage
}

// AIInput locks only source data for a short application transaction; it never
// reads personal notes or supplies credentials/document bytes to an executor.
func (s *Service) Snapshot(ctx context.Context, q core.Querier, a core.Actor, id string) (AIInput, error) {
	var p AIInput
	if core.ID(id) != nil {
		return p, core.Missing
	}
	suffix := ""
	if _, ok := q.(*sql.Tx); ok {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT CAST(id AS CHAR),CAST(created_by AS CHAR),COALESCE(CAST(team_id AS CHAR),''),version,content_hash,normalized_data FROM processes WHERE id=?`+suffix, id).Scan(&p.ID, &p.Owner, &p.Team, &p.Version, &p.Hash, &p.Data)
	if e == sql.ErrNoRows {
		return p, core.Missing
	}
	if e != nil {
		return p, e
	}
	if e = s.Core.Authorize(ctx, q, a, "procesos.view", p.Owner, p.Team, false); e == core.Forbidden {
		e = core.Missing
	}
	return p, e
}

// ScopeSQL is a documented read projection owned by Processes. AI passes only
// a trusted permission name; all values remain query parameters.
func (s *Service) ScopeSQL(a core.Actor, permission string) (string, []any) {
	if permission != "procesos.view" && !strings.HasPrefix(permission, "ai.tasks.") {
		return "FALSE", nil
	}
	return `EXISTS(SELECT 1 FROM processes p JOIN user_roles ur ON ur.user_id=? JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE p.id=t.process_id AND rp.permission=? AND (rp.scope='all' OR (rp.scope='self' AND p.created_by=?) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.user_id=? AND tm.team_id=p.team_id))))`, []any{a.UserID, permission, a.UserID, a.UserID}
}

// EligibleSQL prevents changed inputs or revoked requesters from filling the
// bounded reservation scan. Claim reauthorizes both after owner/task locks.
func (s *Service) EligibleSQL() string {
	clause := `EXISTS(SELECT 1 FROM processes p CROSS JOIN users u WHERE u.id=t.requested_by AND p.id=t.process_id AND p.version=t.input_version AND p.content_hash=t.input_hash AND u.active=TRUE AND u.totp_secret IS NOT NULL`
	for _, permission := range []string{"procesos.view", "ai.tasks.read", "ai.tasks.create"} {
		clause += ` AND EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=t.requested_by AND rp.permission='` + permission + `' AND (rp.scope='all' OR (rp.scope='self' AND p.created_by=t.requested_by) OR (rp.scope='team' AND EXISTS(SELECT 1 FROM team_members tm WHERE tm.user_id=t.requested_by AND tm.team_id=p.team_id))))`
	}
	return clause + `)`
}
