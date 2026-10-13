package collaboration

import (
	"context"
	"database/sql"
	"sort"
	"strconv"
	"strings"
	"time"
	"unicode"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type CommentInput struct {
	Body           string   `json:"body"`
	MentionUserIDs []string `json:"mention_user_ids"`
}

type CommentDeleteInput struct {
	Version uint64 `json:"version,string"`
}

type Comment struct {
	ID             string    `json:"id"`
	ResourceType   string    `json:"resource_type"`
	ResourceID     string    `json:"resource_id"`
	AuthorID       string    `json:"author_id"`
	AuthorName     string    `json:"author_name"`
	Body           string    `json:"body"`
	MentionUserIDs []string  `json:"mention_user_ids"`
	Version        uint64    `json:"version,string"`
	CreatedAt      time.Time `json:"created_at"`
}

type CommentPage struct {
	Data    []Comment `json:"data"`
	Total   int       `json:"total"`
	Page    int       `json:"page"`
	PerPage int       `json:"per_page"`
}

type MentionableUser struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type ActivityItem struct {
	ID           string    `json:"id"`
	ResourceType string    `json:"resource_type"`
	ResourceID   string    `json:"resource_id"`
	ActorID      string    `json:"actor_id"`
	ActorName    string    `json:"actor_name"`
	Action       string    `json:"action"`
	CommentID    string    `json:"comment_id,omitempty"`
	CreatedAt    time.Time `json:"created_at"`
}

type ActivityPage struct {
	Data    []ActivityItem `json:"data"`
	Total   int            `json:"total"`
	Page    int            `json:"page"`
	PerPage int            `json:"per_page"`
}

type CollaborationNotification struct {
	ID           string     `json:"id"`
	ResourceType string     `json:"resource_type"`
	ResourceID   string     `json:"resource_id"`
	CommentID    string     `json:"comment_id"`
	CreatedAt    time.Time  `json:"created_at"`
	ReadAt       *time.Time `json:"read_at,omitempty"`
}

type NotificationPage struct {
	Data        []CollaborationNotification `json:"data"`
	NextAfterID string                      `json:"next_after_id"`
	HasMore     bool                        `json:"has_more"`
}

func (s *Service) Comments(ctx context.Context, a core.Actor, resourceType, resourceID string, page, perPage int) (CommentPage, error) {
	out := CommentPage{Data: []Comment{}, Page: page, PerPage: perPage}
	if !pageBounds(page, perPage) {
		return out, core.Invalid
	}
	resource, e := s.resource(ctx, s.Core.DB, a, resourceType, resourceID)
	if e != nil {
		return out, e
	}
	where := `resource_type=? AND resource_id=? AND deleted_at IS NULL`
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM collaboration_comments WHERE `+where, resourceType, resourceID).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(c.id AS CHAR),c.resource_type,c.resource_id,CAST(c.author_id AS CHAR),u.name,c.body,c.version,c.created_at
		FROM collaboration_comments c JOIN users u ON u.id=c.author_id WHERE c.`+where+` ORDER BY c.created_at,c.id LIMIT ? OFFSET ?`, resourceType, resourceID, perPage, (page-1)*perPage)
	if e != nil {
		return out, e
	}
	for rows.Next() {
		var item Comment
		if e = rows.Scan(&item.ID, &item.ResourceType, &item.ResourceID, &item.AuthorID, &item.AuthorName, &item.Body, &item.Version, &item.CreatedAt); e != nil {
			rows.Close()
			return out, e
		}
		item.MentionUserIDs = []string{}
		out.Data = append(out.Data, item)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return out, e
	}
	if e = s.filterMentionIDs(ctx, s.Core.DB, resource, out.Data); e != nil {
		return out, e
	}
	return out, nil
}

func (s *Service) filterMentionIDs(ctx context.Context, q core.Querier, resource Resource, comments []Comment) error {
	if len(comments) == 0 {
		return nil
	}
	ids := make([]string, 0, len(comments))
	index := make(map[string]int, len(comments))
	for i := range comments {
		ids = append(ids, comments[i].ID)
		index[comments[i].ID] = i
	}
	placeholders := strings.TrimRight(strings.Repeat("?,", len(ids)), ",")
	rows, e := q.QueryContext(ctx, `SELECT CAST(m.comment_id AS CHAR),CAST(u.id AS CHAR)
		FROM collaboration_mentions m JOIN users u ON u.id=m.user_id
		JOIN collaboration_comments c ON c.id=m.comment_id AND c.deleted_at IS NULL
		WHERE m.comment_id IN (`+placeholders+`) AND u.active=TRUE AND u.totp_secret IS NOT NULL
		ORDER BY m.comment_id,u.name,u.id`, stringArgs(ids)...)
	if e != nil {
		return e
	}
	type pair struct{ commentID, userID string }
	pairs := []pair{}
	users := map[string]bool{}
	for rows.Next() {
		var p pair
		if e = rows.Scan(&p.commentID, &p.userID); e != nil {
			rows.Close()
			return e
		}
		pairs = append(pairs, p)
		users[p.userID] = true
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return e
	}
	allowed := map[string]bool{}
	for userID := range users {
		e = s.grantHolderCanResource(ctx, q, userID, resource)
		if e == core.Forbidden {
			continue
		}
		if e != nil {
			return e
		}
		allowed[userID] = true
	}
	for _, p := range pairs {
		if allowed[p.userID] {
			comments[index[p.commentID]].MentionUserIDs = append(comments[index[p.commentID]].MentionUserIDs, p.userID)
		}
	}
	return nil
}

func stringArgs(values []string) []any {
	out := make([]any, len(values))
	for i, value := range values {
		out[i] = value
	}
	return out
}

func (s *Service) CreateComment(ctx context.Context, a core.Actor, resourceType, resourceID, key string, p CommentInput) (any, error) {
	p.Body = strings.TrimSpace(p.Body)
	if a.Type != "user" || len(key) == 0 || len(key) > 100 || !validComment(p.Body) || len(p.MentionUserIDs) > 25 {
		return nil, core.Invalid
	}
	// Normalize into a private slice before sorting: equivalent BIGINT IDs
	// must not create self-notifications/duplicate mentions or mutate caller data.
	p.MentionUserIDs = append([]string(nil), p.MentionUserIDs...)
	for i, userID := range p.MentionUserIDs {
		if core.ID(userID) != nil {
			return nil, core.Invalid
		}
		n, _ := strconv.ParseUint(userID, 10, 64)
		p.MentionUserIDs[i] = strconv.FormatUint(n, 10)
	}
	sort.Strings(p.MentionUserIDs)
	for i, userID := range p.MentionUserIDs {
		if userID == a.UserID || (i > 0 && p.MentionUserIDs[i-1] == userID) {
			return nil, core.Invalid
		}
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := lockCollaborationUsers(ctx, tx, append([]string{a.UserID}, p.MentionUserIDs...)...); e != nil {
			return e
		}
		resource, e := s.resource(ctx, tx, a, resourceType, resourceID)
		if e != nil {
			return e
		}
		if e = resourcePermission(ctx, tx, s, a, resource, "colaboracion.comment", false); e != nil {
			return e
		}
		for _, userID := range p.MentionUserIDs {
			if e = s.grantHolderCanResource(ctx, tx, userID, resource); e != nil {
				if e == core.Forbidden {
					return core.Invalid
				}
				return e
			}
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.comment.create:"+resourceType+":"+resourceID, key, p, func() (any, error) {
			r, e := tx.ExecContext(ctx, `INSERT INTO collaboration_comments(resource_type,resource_id,author_id,body,created_at)
				VALUES(?,?,?,?,UTC_TIMESTAMP(6))`, resourceType, resourceID, a.UserID, p.Body)
			if e != nil {
				return nil, e
			}
			n, e := r.LastInsertId()
			if e != nil {
				return nil, e
			}
			commentID := core.WrapID(n)
			for _, userID := range p.MentionUserIDs {
				if _, e = tx.ExecContext(ctx, `INSERT INTO collaboration_mentions(comment_id,user_id,created_at) VALUES(?,?,UTC_TIMESTAMP(6))`, commentID, userID); e != nil {
					return nil, e
				}
				if _, e = tx.ExecContext(ctx, `INSERT INTO collaboration_notifications(recipient_id,comment_id,created_at) VALUES(?,?,UTC_TIMESTAMP(6))`, userID, commentID); e != nil {
					return nil, e
				}
			}
			if _, e = tx.ExecContext(ctx, `INSERT INTO collaboration_activity(resource_type,resource_id,actor_id,action,comment_id,created_at)
				VALUES(?,?,?,'comment_added',?,UTC_TIMESTAMP(6))`, resourceType, resourceID, a.UserID, commentID); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "collaboration.comment_created", "collaboration_comments", commentID, "success", map[string]any{"resource_type": resourceType, "resource_id": resourceID, "mention_count": len(p.MentionUserIDs)}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "collaboration.comment_created", "collaboration_comments", commentID, 1); e != nil {
				return nil, e
			}
			return map[string]string{"id": commentID, "version": "1"}, nil
		})
		return e
	})
	return out, e
}

func validComment(body string) bool {
	if strings.TrimSpace(body) == "" || len([]rune(body)) > 4000 {
		return false
	}
	for _, r := range body {
		if r == 0 || (unicode.IsControl(r) && r != '\n' && r != '\t') {
			return false
		}
	}
	return true
}

func (s *Service) DeleteComment(ctx context.Context, a core.Actor, id, key string, p CommentDeleteInput) (any, error) {
	if a.Type != "user" || key == "" || len(key) > 100 || core.ID(id) != nil || p.Version == 0 || p.Version == ^uint64(0) {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		var resourceType, resourceID, authorID string
		var version uint64
		var deleted sql.NullTime
		if e := tx.QueryRowContext(ctx, `SELECT resource_type,resource_id,CAST(author_id AS CHAR),version,deleted_at FROM collaboration_comments WHERE id=?`, id).Scan(&resourceType, &resourceID, &authorID, &version, &deleted); e == sql.ErrNoRows {
			return core.Missing
		} else if e != nil {
			return e
		}
		resource, e := s.resource(ctx, tx, a, resourceType, resourceID)
		if e != nil {
			return e
		}
		permission := "colaboracion.moderate"
		if authorID == a.UserID {
			permission = "colaboracion.comment"
		}
		if e = resourcePermission(ctx, tx, s, a, resource, permission, true); e == core.Forbidden && permission == "colaboracion.comment" {
			permission = "colaboracion.moderate"
			e = resourcePermission(ctx, tx, s, a, resource, permission, true)
		}
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "collaboration.comment.delete:"+id, key, p, func() (any, error) {
			var current uint64
			var currentDeleted sql.NullTime
			if e := tx.QueryRowContext(ctx, `SELECT version,deleted_at FROM collaboration_comments WHERE id=? FOR UPDATE`, id).Scan(&current, &currentDeleted); e != nil {
				if e == sql.ErrNoRows {
					return nil, core.Missing
				}
				return nil, e
			}
			if current != p.Version || version != p.Version || currentDeleted.Valid || deleted.Valid {
				return nil, core.Conflict
			}
			if e := core.Changed(tx.ExecContext(ctx, `UPDATE collaboration_comments SET deleted_at=UTC_TIMESTAMP(6),version=version+1 WHERE id=? AND version=? AND deleted_at IS NULL`, id, p.Version)); e != nil {
				return nil, e
			}
			if _, e := tx.ExecContext(ctx, `INSERT INTO collaboration_activity(resource_type,resource_id,actor_id,action,comment_id,created_at)
				VALUES(?,?,?,'comment_removed',?,UTC_TIMESTAMP(6))`, resourceType, resourceID, a.UserID, id); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.comment_removed", "collaboration_comments", id, "success", map[string]string{"resource_type": resourceType, "resource_id": resourceID}); e != nil {
				return nil, e
			}
			if e := s.Core.EmitEvent(ctx, tx, a, "collaboration.comment_removed", "collaboration_comments", id, p.Version+1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id, "version": coreVersion(p.Version + 1)}, nil
		})
		return e
	})
	return out, e
}

func (s *Service) Activity(ctx context.Context, a core.Actor, resourceType, resourceID string, page, perPage int) (ActivityPage, error) {
	out := ActivityPage{Data: []ActivityItem{}, Page: page, PerPage: perPage}
	if !pageBounds(page, perPage) {
		return out, core.Invalid
	}
	if _, e := s.resource(ctx, s.Core.DB, a, resourceType, resourceID); e != nil {
		return out, e
	}
	where := `resource_type=? AND resource_id=?`
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM collaboration_activity WHERE `+where, resourceType, resourceID).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(a.id AS CHAR),a.resource_type,a.resource_id,CAST(a.actor_id AS CHAR),u.name,a.action,CAST(a.comment_id AS CHAR),a.created_at
		FROM collaboration_activity a JOIN users u ON u.id=a.actor_id WHERE a.`+where+` ORDER BY a.created_at,a.id LIMIT ? OFFSET ?`, resourceType, resourceID, perPage, (page-1)*perPage)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var item ActivityItem
		var comment sql.NullString
		if e = rows.Scan(&item.ID, &item.ResourceType, &item.ResourceID, &item.ActorID, &item.ActorName, &item.Action, &comment, &item.CreatedAt); e != nil {
			return out, e
		}
		item.CommentID = comment.String
		out.Data = append(out.Data, item)
	}
	return out, rows.Err()
}

func (s *Service) Notifications(ctx context.Context, a core.Actor, afterID string, limit int) (NotificationPage, error) {
	out := NotificationPage{Data: []CollaborationNotification{}, NextAfterID: "0"}
	if afterID == "" {
		afterID = "0"
	}
	if a.Type != "user" || (afterID != "0" && core.ID(afterID) != nil) || limit < 1 || limit > 100 {
		return out, core.Invalid
	}
	if e := s.Core.Authorize(ctx, s.Core.DB, a, "core.identity.self", a.UserID, "", false); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT CAST(n.id AS CHAR),c.resource_type,c.resource_id,CAST(c.id AS CHAR),n.created_at,n.read_at
		FROM collaboration_notifications n JOIN collaboration_comments c ON c.id=n.comment_id AND c.deleted_at IS NULL
		WHERE n.recipient_id=? AND n.id>? ORDER BY n.id LIMIT ?`, a.UserID, afterID, limit+1)
	if e != nil {
		return out, e
	}
	candidates := []CollaborationNotification{}
	for rows.Next() {
		var item CollaborationNotification
		var readAt sql.NullTime
		if e = rows.Scan(&item.ID, &item.ResourceType, &item.ResourceID, &item.CommentID, &item.CreatedAt, &readAt); e != nil {
			rows.Close()
			return out, e
		}
		if readAt.Valid {
			item.ReadAt = &readAt.Time
		}
		candidates = append(candidates, item)
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return out, e
	}
	out.HasMore = len(candidates) > limit
	if out.HasMore {
		candidates = candidates[:limit]
	}
	out.NextAfterID = afterID
	for _, item := range candidates {
		out.NextAfterID = item.ID
		if _, e = s.resource(ctx, s.Core.DB, a, item.ResourceType, item.ResourceID); e == core.Missing {
			continue
		} else if e != nil {
			return NotificationPage{}, e
		}
		out.Data = append(out.Data, item)
	}
	return out, nil
}

func (s *Service) ReadNotification(ctx context.Context, a core.Actor, id, key string) (any, error) {
	if a.Type != "user" || core.ID(id) != nil || key == "" || len(key) > 100 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.Core.Authorize(ctx, tx, a, "core.identity.self", a.UserID, "", false); e != nil {
			return e
		}
		var resourceType, resourceID, commentID string
		if e := tx.QueryRowContext(ctx, `SELECT c.resource_type,c.resource_id,CAST(c.id AS CHAR) FROM collaboration_notifications n
			JOIN collaboration_comments c ON c.id=n.comment_id AND c.deleted_at IS NULL WHERE n.id=? AND n.recipient_id=? FOR UPDATE`, id, a.UserID).Scan(&resourceType, &resourceID, &commentID); e == sql.ErrNoRows {
			return core.Missing
		} else if e != nil {
			return e
		}
		if _, e := s.resource(ctx, tx, a, resourceType, resourceID); e != nil {
			return e
		}
		var idemErr error
		out, idemErr = s.Core.Idempotent(ctx, tx, a, "collaboration.notification.read:"+id, key, map[string]bool{"read": true}, func() (any, error) {
			if _, e := tx.ExecContext(ctx, `UPDATE collaboration_notifications SET read_at=COALESCE(read_at,UTC_TIMESTAMP(6)) WHERE id=? AND recipient_id=?`, id, a.UserID); e != nil {
				return nil, e
			}
			if e := s.Core.Audit(ctx, tx, a, "collaboration.notification_read", "collaboration_notifications", id, "success", map[string]string{"comment_id": commentID}); e != nil {
				return nil, e
			}
			return map[string]any{"id": id, "read": true}, nil
		})
		return idemErr
	})
	return out, e
}
