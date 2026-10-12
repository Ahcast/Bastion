package collaboration

import (
	"context"
	"database/sql"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

var Permissions = []string{
	"colaboracion.view",
	"colaboracion.upload",
	"colaboracion.organize",
	"colaboracion.delete",
	"colaboracion.comment",
	"colaboracion.moderate",
}

type Resource struct {
	ID              string
	OwnerID         string
	TeamID          string
	DocumentID      string
	DocumentOwnerID string
	DocumentTeamID  string
}

type ResourcePolicy func(context.Context, core.Querier, string) (Resource, error)

type Service struct {
	Core             *core.Service
	ResourcePolicies map[string]ResourcePolicy
}

type Folder struct {
	ID        string     `json:"id"`
	Scope     string     `json:"scope"`
	PersonID  string     `json:"person_id,omitempty"`
	TeamID    string     `json:"team_id,omitempty"`
	ParentID  string     `json:"parent_id,omitempty"`
	Name      string     `json:"name"`
	CreatedBy string     `json:"created_by"`
	Version   uint64     `json:"version,string"`
	FileCount int        `json:"file_count"`
	CreatedAt time.Time  `json:"created_at"`
	UpdatedAt time.Time  `json:"updated_at"`
	DeletedAt *time.Time `json:"deleted_at,omitempty"`
}

type FolderInput struct {
	Scope    string `json:"scope"`
	PersonID string `json:"person_id"`
	TeamID   string `json:"team_id"`
	ParentID string `json:"parent_id"`
	Name     string `json:"name"`
}

type VersionInput struct {
	Version uint64 `json:"version,string"`
}

type FolderUpdate struct {
	Version uint64 `json:"version,string"`
	Name    string `json:"name"`
}

type EntryInput struct {
	DocumentID      string `json:"document_id"`
	DocumentVersion uint64 `json:"document_version,string"`
	Scope           string `json:"scope"`
	PersonID        string `json:"person_id"`
	FolderID        string `json:"folder_id"`
	Title           string `json:"title"`
	Notes           string `json:"notes"`
}

type EntryUpdate struct {
	Version  uint64 `json:"version,string"`
	FolderID string `json:"folder_id"`
	Title    string `json:"title"`
	Notes    string `json:"notes"`
}

type Entry struct {
	ID              string    `json:"id"`
	DocumentID      string    `json:"document_id"`
	FolderID        string    `json:"folder_id,omitempty"`
	Scope           string    `json:"scope"`
	PersonID        string    `json:"person_id,omitempty"`
	TeamID          string    `json:"team_id,omitempty"`
	Title           string    `json:"title"`
	Notes           string    `json:"notes,omitempty"`
	UploadedBy      string    `json:"uploaded_by"`
	Version         uint64    `json:"version,string"`
	DocumentVersion uint64    `json:"document_version,string"`
	FileVersion     int       `json:"file_version"`
	Filename        string    `json:"filename"`
	MediaType       string    `json:"media_type"`
	SizeBytes       int64     `json:"size_bytes"`
	Status          string    `json:"status"`
	UpdatedAt       time.Time `json:"updated_at"`
	Deleted         bool      `json:"deleted"`
}

type Filter struct {
	Scope     string
	PersonID  string
	FolderID  string
	FolderSet bool
	Search    string
	Deleted   bool
	Page      int
	PerPage   int
}

type Page struct {
	Data    []Entry `json:"data"`
	Total   int     `json:"total"`
	Page    int     `json:"page"`
	PerPage int     `json:"per_page"`
}

type Team struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type entryRow struct {
	Entry
	Folder sql.NullString
	Person sql.NullString
	Team   sql.NullString
}

func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, permission, owner, team string, sensitive bool) error {
	if a.Type != "user" {
		return core.Forbidden
	}
	if e := s.Core.Authorize(ctx, q, a, "colaboracion.view", owner, team, false); e != nil {
		return e
	}
	if permission == "colaboracion.view" {
		return nil
	}
	return s.Core.Authorize(ctx, q, a, permission, owner, team, sensitive)
}

// authorizeAny validates a collection-level permission and the current MFA
// session; row-level scopes are still applied in SQL before pagination.
func (s *Service) authorizeAny(ctx context.Context, q core.Querier, a core.Actor, permission string) error {
	if a.Type != "user" {
		return core.Forbidden
	}
	var team string
	e := q.QueryRowContext(ctx, `SELECT CAST(team_id AS CHAR) FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, permission, a.UserID, team, false)
}

func (s *Service) DocumentPolicy(ctx context.Context, q core.Querier, a core.Actor, id string, _ bool) error {
	if core.ID(id) != nil {
		return core.Missing
	}
	var scope, uploadedBy string
	var person, team, folder, folderScope, folderPerson, folderTeam sql.NullString
	var folderDeleted sql.NullTime
	var documentOwner string
	var documentTeam, documentEntity, documentEntityID sql.NullString
	var generated bool
	e := q.QueryRowContext(ctx, `SELECT e.scope,CAST(e.uploaded_by AS CHAR),CAST(e.person_id AS CHAR),CAST(e.team_id AS CHAR),CAST(e.folder_id AS CHAR),
		f.scope,CAST(f.person_id AS CHAR),CAST(f.team_id AS CHAR),f.deleted_at,
		d.owner_id,d.team_id,d.entity_type,d.entity_id,d.is_generated
		FROM collaboration_entries e JOIN documents d ON d.id=e.document_id LEFT JOIN collaboration_folders f ON f.id=e.folder_id WHERE e.id=?`, id).Scan(
		&scope, &uploadedBy, &person, &team, &folder, &folderScope, &folderPerson, &folderTeam, &folderDeleted,
		&documentOwner, &documentTeam, &documentEntity, &documentEntityID, &generated)
	if e == sql.ErrNoRows {
		return core.Missing
	}
	if e != nil {
		return e
	}
	owner := uploadedBy
	if scope == "person" {
		if !person.Valid || person.String != uploadedBy || documentOwner != uploadedBy || team.Valid || documentTeam.Valid {
			return core.Forbidden
		}
		owner = person.String
	} else if scope != "company" || person.Valid || documentOwner != uploadedBy || documentTeam.String != team.String {
		return core.Forbidden
	}
	if generated || documentEntity.String != "collaboration" || documentEntityID.String != id {
		// BindDocument calls the destination policy before writing the link. That
		// one transition starts with an unbound, live document.
		if documentEntity.Valid || generated {
			return core.Missing
		}
	}
	if folder.Valid {
		if !folderScope.Valid || folderScope.String != scope || folderPerson.String != person.String || folderTeam.String != team.String || folderDeleted.Valid {
			return core.Missing
		}
	}
	if e = s.Core.Authorize(ctx, q, a, "colaboracion.view", owner, team.String, false); e != nil {
		return e
	}
	return nil
}

func (s *Service) DocumentBindingPolicy(ctx context.Context, q core.Querier, a core.Actor, id string) error {
	r, e := s.entry(ctx, q, a, id, "colaboracion.organize")
	if e != nil {
		return e
	}
	return s.gate(ctx, q, a, "colaboracion.organize", r.UploadedBy, r.TeamID, true)
}

func (s *Service) DocumentVersionPolicy(ctx context.Context, q core.Querier, a core.Actor, id string) error {
	r, e := s.entry(ctx, q, a, id, "colaboracion.upload")
	if e != nil {
		return e
	}
	return s.gate(ctx, q, a, "colaboracion.upload", r.UploadedBy, r.TeamID, false)
}

// DocumentTrashPolicy prevents a caller with generic Core.Files write access
// from bypassing the module's explicit delete grant.
func (s *Service) DocumentTrashPolicy(ctx context.Context, q core.Querier, a core.Actor, id string) error {
	r, e := s.entry(ctx, q, a, id, "colaboracion.delete")
	if e != nil {
		return e
	}
	return s.gate(ctx, q, a, "colaboracion.delete", r.UploadedBy, r.TeamID, true)
}

func (s *Service) folder(ctx context.Context, q core.Querier, a core.Actor, id string, permission string, allowDeleted bool) (Folder, error) {
	var f Folder
	if core.ID(id) != nil {
		return f, core.Invalid
	}
	var person, team, parent sql.NullString
	var deleted sql.NullTime
	suffix := ""
	if _, ok := q.(*sql.Tx); ok {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT CAST(id AS CHAR),scope,CAST(person_id AS CHAR),CAST(team_id AS CHAR),CAST(parent_id AS CHAR),name,CAST(created_by AS CHAR),version,created_at,updated_at,deleted_at
		FROM collaboration_folders WHERE id=?`+suffix, id).Scan(&f.ID, &f.Scope, &person, &team, &parent, &f.Name, &f.CreatedBy, &f.Version, &f.CreatedAt, &f.UpdatedAt, &deleted)
	if e == sql.ErrNoRows {
		return f, core.Missing
	}
	if e != nil {
		return f, e
	}
	f.PersonID, f.TeamID, f.ParentID = person.String, team.String, parent.String
	if deleted.Valid {
		f.DeletedAt = &deleted.Time
		if !allowDeleted {
			return f, core.Missing
		}
	}
	owner := f.CreatedBy
	if f.Scope == "person" {
		if f.PersonID == "" {
			return f, core.Forbidden
		}
		owner = f.PersonID
	} else if f.Scope != "company" || f.PersonID != "" {
		return f, core.Forbidden
	}
	if e = s.Core.Authorize(ctx, q, a, "colaboracion.view", owner, f.TeamID, false); e != nil {
		return f, core.Missing
	}
	if permission != "colaboracion.view" {
		if e = s.Core.Authorize(ctx, q, a, permission, owner, f.TeamID, permission == "colaboracion.delete"); e != nil {
			return f, e
		}
	}
	return f, nil
}

func (s *Service) entry(ctx context.Context, q core.Querier, a core.Actor, id string, permission string) (entryRow, error) {
	var r entryRow
	if core.ID(id) != nil {
		return r, core.Invalid
	}
	e := q.QueryRowContext(ctx, `SELECT CAST(id AS CHAR),CAST(document_id AS CHAR),CAST(folder_id AS CHAR),scope,CAST(person_id AS CHAR),CAST(team_id AS CHAR),title,COALESCE(notes,''),CAST(uploaded_by AS CHAR),version,updated_at
		FROM collaboration_entries WHERE id=?`, id).Scan(&r.ID, &r.DocumentID, &r.Folder, &r.Scope, &r.Person, &r.Team, &r.Title, &r.Notes, &r.UploadedBy, &r.Version, &r.UpdatedAt)
	if e == sql.ErrNoRows {
		return r, core.Missing
	}
	if e != nil {
		return r, e
	}
	r.FolderID, r.PersonID, r.TeamID = r.Folder.String, r.Person.String, r.Team.String
	owner := r.UploadedBy
	if r.Scope == "person" {
		if r.PersonID != r.UploadedBy {
			return r, core.Missing
		}
		owner = r.PersonID
	} else if r.Scope != "company" || r.Person.Valid {
		return r, core.Missing
	}
	if e = s.gate(ctx, q, a, permission, owner, r.TeamID, false); e != nil {
		if e == core.Forbidden {
			return r, core.Missing
		}
		return r, e
	}
	if r.Folder.Valid {
		var deleted sql.NullTime
		if e = q.QueryRowContext(ctx, `SELECT deleted_at FROM collaboration_folders WHERE id=?`, r.FolderID).Scan(&deleted); e == sql.ErrNoRows {
			return r, core.Missing
		} else if e != nil {
			return r, e
		} else if deleted.Valid {
			return r, core.Missing
		}
	}
	return r, nil
}

func (s *Service) Entry(ctx context.Context, a core.Actor, id string) (Entry, error) {
	return s.entryWithMetadata(ctx, s.Core.DB, a, id, false)
}

// entryWithMetadata can read from either the connection pool or a transaction.
// Mutations use the transaction so their response reflects the row they just
// wrote without depending on an uncommitted read through the pool.
func (s *Service) entryWithMetadata(ctx context.Context, q core.Querier, a core.Actor, id string, deleted bool) (Entry, error) {
	r, e := s.entry(ctx, q, a, id, "colaboracion.view")
	if e != nil {
		return Entry{}, e
	}
	m, e := s.Core.DocumentMetadata(ctx, q, a, r.DocumentID)
	if e != nil {
		return Entry{}, e
	}
	return withMetadata(r.Entry, m, deleted), nil
}

func withMetadata(e Entry, m core.DocumentMetadata, deleted bool) Entry {
	e.DocumentVersion = m.Version
	e.FileVersion = m.Number
	e.Filename = m.Filename
	e.MediaType = m.Media
	e.SizeBytes = m.Size
	e.Status = m.Status
	e.Deleted = deleted
	return e
}

func (s *Service) Versions(ctx context.Context, a core.Actor, id string) (any, error) {
	r, e := s.entry(ctx, s.Core.DB, a, id, "colaboracion.view")
	if e != nil {
		return nil, e
	}
	return s.Core.DocumentVersions(ctx, s.Core.DB, a, r.DocumentID)
}

func (s *Service) Teams(ctx context.Context, a core.Actor) ([]Team, error) {
	if e := s.authorizeAny(ctx, s.Core.DB, a, "colaboracion.view"); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(t.id AS CHAR),t.name FROM teams t JOIN team_members tm ON tm.team_id=t.id WHERE tm.user_id=? ORDER BY t.name,t.id`, a.UserID)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Team{}
	for rows.Next() {
		var team Team
		if e = rows.Scan(&team.ID, &team.Name); e != nil {
			return nil, e
		}
		out = append(out, team)
	}
	return out, rows.Err()
}

func validName(v string, max int) bool {
	if strings.ContainsAny(v, "\r\n\x00") {
		return false
	}
	v = strings.TrimSpace(v)
	return len(v) > 0 && len([]rune(v)) <= max
}

func pageBounds(page, perPage int) bool {
	return page >= 1 && page <= 100000 && perPage >= 1 && perPage <= 100
}

func nullableString(v string) any {
	if v == "" {
		return nil
	}
	return v
}
