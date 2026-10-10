package workflow

import (
	"context"
	"database/sql"
	"encoding/hex"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"strings"
	"time"
	"unicode/utf8"
)

type Service struct{ Core *core.Service }

func (s *Service) Module(enabled bool) registry.Module {
	return registry.Module{ID: "workflow", Label: "Flujos y aprobaciones", Version: "0.1.0", Enabled: enabled, Dependencies: []string{"core"}}
}

// Binding is built by a business service, never accepted from HTTP or an agent.
type Binding struct {
	Action             string `json:"action"`
	Entity             string `json:"entity_type"`
	ID                 string `json:"entity_id"`
	Version            uint64 `json:"entity_version,string"`
	Hash               string `json:"input_hash"`
	Owner              string `json:"-"`
	Team               string `json:"-"`
	ViewPermission     string `json:"-"`
	RequestPermission  string `json:"-"`
	DecisionPermission string `json:"-"`
	RevokePermission   string `json:"-"`
}
type Approval struct {
	Binding
	ApprovalID  string    `json:"id"`
	RequestedBy string    `json:"requested_by"`
	DecidedBy   string    `json:"decided_by"`
	Status      string    `json:"status"`
	Reason      string    `json:"reason"`
	Expires     time.Time `json:"expires_at"`
	Version     uint64    `json:"version,string"`
}

func ReasonValid(reason string, required bool) bool {
	return utf8.ValidString(reason) && utf8.RuneCountInString(reason) <= 1000 && !strings.ContainsAny(reason, "\x00") && (!required || strings.TrimSpace(reason) != "")
}
func bindingValid(b Binding) bool {
	h, e := hex.DecodeString(b.Hash)
	return e == nil && len(h) == 32 && core.ID(b.ID) == nil && core.ID(b.Owner) == nil && b.Version > 0 && len(b.Action) > 0 && len(b.Action) <= 80 && len(b.Entity) > 0 && len(b.Entity) <= 60 && b.ViewPermission != "" && b.RequestPermission != "" && b.DecisionPermission != "" && b.RevokePermission != ""
}

const approvalColumns = `id,action,entity_type,entity_id,entity_version,input_hash,owner_id,team_id,view_permission,request_permission,decision_permission,revoke_permission,requested_by,COALESCE(CAST(decided_by AS CHAR),''),status,reason,expires_at,version`

func (s *Service) Get(ctx context.Context, tx *sql.Tx, id, entity, eid string) (Approval, error) {
	var v Approval
	if core.ID(id) != nil {
		return v, core.Invalid
	}
	e := tx.QueryRowContext(ctx, `SELECT `+approvalColumns+` FROM approvals WHERE id=? AND entity_type=? AND entity_id=? FOR UPDATE`, id, entity, eid).Scan(&v.ApprovalID, &v.Action, &v.Entity, &v.ID, &v.Binding.Version, &v.Hash, &v.Owner, &v.Team, &v.ViewPermission, &v.RequestPermission, &v.DecisionPermission, &v.RevokePermission, &v.RequestedBy, &v.DecidedBy, &v.Status, &v.Reason, &v.Expires, &v.Version)
	if e == sql.ErrNoRows {
		return v, core.Missing
	}
	return v, e
}

// List applies the current aggregate scope supplied by its owning service.
func (s *Service) List(ctx context.Context, tx *sql.Tx, a core.Actor, scope Binding, page, limit int) ([]Approval, int, error) {
	out := []Approval{}
	var total int
	if page < 1 || page > 100000 || limit < 1 || limit > 100 || core.ID(scope.ID) != nil || scope.ViewPermission == "" {
		return nil, 0, core.Invalid
	}
	if e := s.Core.Authorize(ctx, tx, a, scope.ViewPermission, scope.Owner, scope.Team, false); e != nil {
		return nil, 0, e
	}
	if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM approvals WHERE entity_type=? AND entity_id=?`, scope.Entity, scope.ID).Scan(&total); e != nil {
		return nil, 0, e
	}
	rows, e := tx.QueryContext(ctx, `SELECT id FROM approvals WHERE entity_type=? AND entity_id=? ORDER BY id DESC LIMIT ? OFFSET ?`, scope.Entity, scope.ID, limit, (page-1)*limit)
	if e != nil {
		return nil, 0, e
	}
	ids := []string{}
	for rows.Next() {
		var id string
		if e = rows.Scan(&id); e != nil {
			rows.Close()
			return nil, 0, e
		}
		ids = append(ids, id)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return nil, 0, e
	}
	for _, id := range ids {
		v, e := s.Get(ctx, tx, id, scope.Entity, scope.ID)
		if e != nil {
			return nil, 0, e
		}
		out = append(out, v)
	}
	return out, total, nil
}
func (s *Service) Request(ctx context.Context, tx *sql.Tx, a core.Actor, b Binding, expires time.Time, reason string) (Approval, error) {
	var v Approval
	if a.Type != "user" {
		return v, core.Forbidden
	}
	if !bindingValid(b) || !ReasonValid(reason, false) || !expires.After(time.Now()) || expires.Year() > 9999 {
		return v, core.Invalid
	}
	if e := s.Core.Authorize(ctx, tx, a, b.RequestPermission, b.Owner, b.Team, false); e != nil {
		return v, e
	}
	if e := s.Core.Authorize(ctx, tx, a, b.ViewPermission, b.Owner, b.Team, false); e != nil {
		return v, e
	}
	var count int
	if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM approvals WHERE action=? AND entity_type=? AND entity_id=? AND entity_version=?`, b.Action, b.Entity, b.ID, b.Version).Scan(&count); e != nil {
		return v, e
	}
	if count != 0 {
		return v, core.Conflict
	}
	r, e := tx.ExecContext(ctx, `INSERT INTO approvals(action,entity_type,entity_id,entity_version,input_hash,owner_id,team_id,view_permission,request_permission,decision_permission,revoke_permission,requested_by,status,reason,expires_at,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,'pending',?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`, b.Action, b.Entity, b.ID, b.Version, b.Hash, b.Owner, b.Team, b.ViewPermission, b.RequestPermission, b.DecisionPermission, b.RevokePermission, a.UserID, reason, expires.UTC().Truncate(time.Microsecond))
	if e != nil {
		return v, e
	}
	id, e := r.LastInsertId()
	if e != nil {
		return v, e
	}
	v = Approval{Binding: b, ApprovalID: core.WrapID(id), RequestedBy: a.UserID, Status: "pending", Reason: reason, Expires: expires.UTC().Truncate(time.Microsecond), Version: 1}
	return v, s.record(ctx, tx, a, v)
}
func sameBinding(v Approval, b Binding) bool {
	return v.Action == b.Action && v.Entity == b.Entity && v.ID == b.ID && v.Binding.Version == b.Version && v.Hash == b.Hash && v.Owner == b.Owner && v.Team == b.Team && v.ViewPermission == b.ViewPermission && v.DecisionPermission == b.DecisionPermission && v.RequestPermission == b.RequestPermission && v.RevokePermission == b.RevokePermission
}
func (s *Service) Decide(ctx context.Context, tx *sql.Tx, a core.Actor, v Approval, b Binding, version uint64, decision, reason string) (Approval, error) {
	if a.Type != "user" || a.UserID == v.RequestedBy || a.UserID == v.Owner {
		return v, core.Forbidden
	}
	if (decision != "approved" && decision != "rejected") || !ReasonValid(reason, true) {
		return v, core.Invalid
	}
	if e := s.Core.Authorize(ctx, tx, a, b.DecisionPermission, b.Owner, b.Team, true); e != nil {
		return v, e
	}
	if e := s.Core.Authorize(ctx, tx, a, b.ViewPermission, b.Owner, b.Team, false); e != nil {
		return v, e
	}
	if v.Status != "pending" || v.Version != version || !sameBinding(v, b) || !v.Expires.After(time.Now()) {
		return v, core.Conflict
	}
	v.DecidedBy = a.UserID
	return s.change(ctx, tx, a, v, decision, reason)
}
func (s *Service) Revoke(ctx context.Context, tx *sql.Tx, a core.Actor, v Approval, version uint64, reason string) (Approval, error) {
	if a.Type != "user" {
		return v, core.Forbidden
	}
	if !ReasonValid(reason, true) {
		return v, core.Invalid
	}
	if e := s.Core.Authorize(ctx, tx, a, v.RevokePermission, v.Owner, v.Team, true); e != nil {
		return v, e
	}
	if e := s.Core.Authorize(ctx, tx, a, v.ViewPermission, v.Owner, v.Team, false); e != nil {
		return v, e
	}
	if (v.Status != "pending" && v.Status != "approved") || v.Version != version {
		return v, core.Conflict
	}
	return s.change(ctx, tx, a, v, "revoked", reason)
}
func approvalError(code, message string) error {
	return &core.Error{Code: code, Message: message, Status: 409}
}

// Verify rechecks the decision holder's live authority independently of the
// original session. The caller separately authorizes execution and its actor.
func (s *Service) Verify(ctx context.Context, tx *sql.Tx, v Approval, b Binding) error {
	if v.Status != "approved" {
		return approvalError("approval_not_active", "La aprobación no está activa.")
	}
	if !sameBinding(v, b) {
		return approvalError("approval_changed", "La versión o el contenido aprobado cambió.")
	}
	if !v.Expires.After(time.Now()) {
		return approvalError("approval_expired", "La aprobación venció.")
	}
	if v.DecidedBy == "" || v.DecidedBy == v.RequestedBy || v.DecidedBy == v.Owner {
		return core.Forbidden
	}
	for _, permission := range []string{b.DecisionPermission, b.ViewPermission} {
		if e := s.Core.AuthorizeGrantHolder(ctx, tx, v.DecidedBy, permission, b.Owner, b.Team); e != nil {
			if e == core.Forbidden {
				return approvalError("approval_authority_revoked", "El aprobador ya no tiene autorización vigente.")
			}
			return e
		}
	}
	return nil
}

// Consume must share the transaction of the business effect. It grants no
// external delivery; the caller supplies its registered execution permission.
func (s *Service) Consume(ctx context.Context, tx *sql.Tx, a core.Actor, id string, b Binding, executionPermission string) error {
	if a.Type != "user" {
		return core.Forbidden
	}
	registered := false
	for _, p := range s.Core.PermissionCatalog() {
		if p == executionPermission {
			registered = true
		}
	}
	if !registered {
		return core.Forbidden
	}
	if e := s.Core.Authorize(ctx, tx, a, executionPermission, b.Owner, b.Team, true); e != nil {
		return e
	}
	if e := s.Core.Authorize(ctx, tx, a, b.ViewPermission, b.Owner, b.Team, false); e != nil {
		return e
	}
	v, e := s.Get(ctx, tx, id, b.Entity, b.ID)
	if e != nil {
		return e
	}
	if e = s.Verify(ctx, tx, v, b); e != nil {
		return e
	}
	_, e = s.change(ctx, tx, a, v, "consumed", "Acción autorizada ejecutada")
	return e
}
func (s *Service) Invalidate(ctx context.Context, tx *sql.Tx, a core.Actor, entity, id, reason string) error {
	rows, e := tx.QueryContext(ctx, `SELECT id FROM approvals WHERE entity_type=? AND entity_id=? AND status IN ('pending','approved') ORDER BY id FOR UPDATE`, entity, id)
	if e != nil {
		return e
	}
	ids := []string{}
	for rows.Next() {
		var id string
		if e = rows.Scan(&id); e != nil {
			rows.Close()
			return e
		}
		ids = append(ids, id)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return e
	}
	// IDs are collected before writes so database/sql never holds a result reader
	// while another statement needs the same transaction connection.
	for _, approvalID := range ids {
		v, e := s.Get(ctx, tx, approvalID, entity, id)
		if e != nil {
			return e
		}
		if _, e = s.change(ctx, tx, a, v, "invalidated", reason); e != nil {
			return e
		}
	}
	return nil
}
func (s *Service) change(ctx context.Context, tx *sql.Tx, a core.Actor, v Approval, status, reason string) (Approval, error) {
	if e := core.Changed(tx.ExecContext(ctx, `UPDATE approvals SET status=?,reason=?,decided_by=?,version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`, status, reason, nullUser(v.DecidedBy), v.ApprovalID, v.Version)); e != nil {
		return v, e
	}
	v.Status = status
	v.Reason = reason
	v.Version++
	return v, s.record(ctx, tx, a, v)
}
func nullUser(s string) any {
	if s == "" {
		return nil
	}
	return s
}
func (s *Service) record(ctx context.Context, tx *sql.Tx, a core.Actor, v Approval) error {
	if _, e := tx.ExecContext(ctx, `INSERT INTO approval_events VALUES(?,?,?,?,?,UTC_TIMESTAMP(6))`, v.ApprovalID, v.Version, v.Status, v.Reason, a.UserID); e != nil {
		return e
	}
	if e := s.Core.Audit(ctx, tx, a, "approval."+v.Status, "approvals", v.ApprovalID, "success", map[string]string{"version": fmt.Sprint(v.Version), "entity_type": v.Entity, "entity_id": v.ID}); e != nil {
		return e
	}
	return s.Core.EmitEvent(ctx, tx, a, "approval."+v.Status, "approvals", v.ApprovalID, v.Version)
}
