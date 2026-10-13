package ai

import (
	"context"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"regexp"
	"strconv"
	"time"
)

var Permissions = []string{"ai.tasks.read", "ai.tasks.create", "ai.tasks.execute", "ai.tasks.cancel"}
var LeaseExpired = &core.Error{Code: "lease_expired", Message: "La reserva venció o fue reemplazada.", Status: 409}
var InputStale = &core.Error{Code: "input_stale", Message: "El proceso cambió. Revise la entrada de la tarea.", Status: 409}

// Source is supplied by the owning module. ScopeSQL is application-authored,
// parameterized SQL for live owner permissions, never an expression from a client.
type Source interface {
	Snapshot(context.Context, core.Querier, core.Actor, string) (SourceInput, error)
	ScopeSQL(core.Actor, string) (string, []any)
	EligibleSQL() string
}
type SourceInput = processes.AIInput
type Service struct {
	Core      *core.Service
	Processes Source
}
type CreateInput struct {
	Schema       string `json:"schema_version"`
	ProcessID    string `json:"process_id"`
	InputVersion uint64 `json:"input_version,string"`
	SkillVersion string `json:"skill_version"`
}
type ClaimInput struct {
	Schema string `json:"schema_version"`
	Limit  int    `json:"limit"`
}
type ExecutionInput struct {
	Schema       string `json:"schema_version"`
	Version      uint64 `json:"version,string"`
	RunID        string `json:"run_id"`
	InputVersion uint64 `json:"input_version,string"`
	InputHash    string `json:"input_hash"`
	SkillVersion string `json:"skill_version"`
	Lease        string `json:"lease_token"`
}
type FailInput struct {
	ExecutionInput
	Code string `json:"error_code"`
}
type CancelInput struct {
	Schema  string `json:"schema_version"`
	Version uint64 `json:"version,string"`
}
type Filter struct {
	Page      int    `json:"page,omitempty"`
	PerPage   int    `json:"per_page,omitempty"`
	ProcessID string `json:"process_id,omitempty"`
	Status    string `json:"status,omitempty"`
}
type Task struct {
	ID           string    `json:"id"`
	Type         string    `json:"task_type"`
	ProcessID    string    `json:"process_id"`
	InputVersion uint64    `json:"input_version,string"`
	InputHash    string    `json:"input_hash"`
	SkillVersion string    `json:"skill_version"`
	RequestedBy  string    `json:"requested_by"`
	Status       string    `json:"status"`
	Version      uint64    `json:"version,string"`
	Attempts     int       `json:"attempts"`
	Available    time.Time `json:"available_at"`
	CurrentRun   string    `json:"current_run_id"`
	Created      time.Time `json:"created_at"`
	Updated      time.Time `json:"updated_at"`
}
type Run struct {
	ID           string     `json:"id"`
	Attempt      int        `json:"attempt"`
	Status       string     `json:"status"`
	SkillVersion string     `json:"skill_version"`
	Expires      time.Time  `json:"lease_expires_at"`
	Renewals     int        `json:"renewals"`
	Started      time.Time  `json:"started_at"`
	Finished     *time.Time `json:"finished_at"`
	Code         *string    `json:"error_code"`
}
type Detail struct {
	Task
	CurrentInput        bool   `json:"current_input"`
	CurrentInputVersion uint64 `json:"current_input_version,string"`
	Runs                []Run  `json:"runs"`
}
type Listing struct {
	Data    []Task `json:"data"`
	Total   int    `json:"total"`
	Page    int    `json:"page,omitempty"`
	PerPage int    `json:"per_page,omitempty"`
}
type Reservation struct {
	Task    Task            `json:"task"`
	RunID   string          `json:"run_id"`
	Lease   string          `json:"lease_token"`
	Expires time.Time       `json:"lease_expires_at"`
	Input   json.RawMessage `json:"input"`
	Warning string          `json:"warning"`
}
type Claims struct {
	Data []Reservation `json:"data"`
}

var skill = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._:/-]{0,99}$`)
var hash = regexp.MustCompile(`^[a-f0-9]{64}$`)

func canonical(id string) bool {
	n, e := strconv.ParseUint(id, 10, 64)
	return e == nil && n > 0 && strconv.FormatUint(n, 10) == id
}
func validExecution(p ExecutionInput) bool {
	return p.Schema == "1.0" && p.Version > 0 && canonical(p.RunID) && p.InputVersion > 0 && hash.MatchString(p.InputHash) && skill.MatchString(p.SkillVersion) && len(p.Lease) == 43
}
func validStatus(s string) bool {
	return s == "pending" || s == "running" || s == "failed" || s == "needs_review" || s == "cancelled"
}
