package quotes

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/csv"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

const maxBulkItems = 100

type BulkItemInput struct {
	ID      string `json:"id"`
	Version uint64 `json:"version,string"`
}

type BulkStatusInput struct {
	Items  []BulkItemInput `json:"items"`
	Status string          `json:"status"`
}

type BulkCloneInput struct {
	Items []BulkItemInput `json:"items"`
}

type BulkExportInput struct {
	Items []BulkItemInput `json:"items"`
}

type BulkItemResult struct {
	Position int             `json:"position"`
	ID       string          `json:"id"`
	Version  uint64          `json:"version,string"`
	Status   string          `json:"status"`
	Error    string          `json:"error_code,omitempty"`
	Result   json.RawMessage `json:"result,omitempty"`
}

type BulkResponse struct {
	BatchID  string           `json:"batch_id"`
	Action   string           `json:"action"`
	Status   string           `json:"status"`
	Results  []BulkItemResult `json:"results"`
	Filename string           `json:"filename,omitempty"`
	Content  string           `json:"content,omitempty"`
}

type bulkPendingItem struct {
	Position int
	ID       string
	Version  uint64
}

func validateBulkItems(items []BulkItemInput) error {
	if len(items) == 0 || len(items) > maxBulkItems {
		return core.Invalid
	}
	seen := make(map[string]struct{}, len(items))
	for _, item := range items {
		if core.ID(item.ID) != nil || item.Version == 0 {
			return core.Invalid
		}
		if _, ok := seen[item.ID]; ok {
			return core.Invalid
		}
		seen[item.ID] = struct{}{}
	}
	return nil
}

func bulkPermission(action string) (string, error) {
	switch action {
	case "status":
		return "cotizaciones.transition", nil
	case "clone":
		return "cotizaciones.create", nil
	case "export":
		return "cotizaciones.view", nil
	default:
		return "", core.Invalid
	}
}

func (s *Service) createBulkBatch(ctx context.Context, a core.Actor, key, action, target string, items []BulkItemInput) (string, error) {
	if a.Type != "user" {
		return "", core.Forbidden
	}
	if e := validateBulkItems(items); e != nil {
		return "", e
	}
	perm, e := bulkPermission(action)
	if e != nil {
		return "", e
	}
	input := struct {
		Target string          `json:"target_status"`
		Items  []BulkItemInput `json:"items"`
	}{Target: target, Items: items}
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, perm); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.bulk."+action, key, input, func() (any, error) {
			res, e := tx.ExecContext(ctx, `INSERT INTO quote_batches(action,requested_by,target_status,status,created_at) VALUES(?,?,?,'processing',UTC_TIMESTAMP(6))`, action, a.UserID, target)
			if e != nil {
				return nil, e
			}
			id, e := res.LastInsertId()
			if e != nil {
				return nil, e
			}
			batch := core.WrapID(id)
			for i, item := range items {
				if _, e = tx.ExecContext(ctx, `INSERT INTO quote_batch_items(batch_id,position,quote_id,quote_version,item_status,error_code,result_json,updated_at) VALUES(?,?,?,?,'pending','',NULL,UTC_TIMESTAMP(6))`, batch, i, item.ID, item.Version); e != nil {
					return nil, e
				}
			}
			if e = s.Core.Audit(ctx, tx, a, "quote.bulk_created", "quote_batches", batch, "success", map[string]any{"action": action, "items": len(items), "target_status": target}); e != nil {
				return nil, e
			}
			return map[string]string{"batch_id": batch}, nil
		})
		return e
	})
	if e != nil {
		return "", e
	}
	encoded, e := json.Marshal(out)
	if e != nil {
		return "", e
	}
	var result struct {
		BatchID string `json:"batch_id"`
	}
	if e = json.Unmarshal(encoded, &result); e != nil || core.ID(result.BatchID) != nil {
		return "", core.Conflict
	}
	return result.BatchID, nil
}

func bulkErrorCode(e error) string {
	var ce *core.Error
	if errors.As(e, &ce) && ce.Code != "" {
		return ce.Code
	}
	return "internal"
}

func (s *Service) pendingBulkItems(ctx context.Context, batch string) ([]bulkPendingItem, error) {
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT position,CAST(quote_id AS CHAR),quote_version FROM quote_batch_items WHERE batch_id=? AND item_status='pending' ORDER BY position`, batch)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []bulkPendingItem{}
	for rows.Next() {
		var item bulkPendingItem
		if e = rows.Scan(&item.Position, &item.ID, &item.Version); e != nil {
			return nil, e
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (s *Service) settleBulkItem(ctx context.Context, batch string, position int, status, errorCode string, result any) error {
	var raw any
	if result != nil {
		encoded, e := json.Marshal(result)
		if e != nil {
			return e
		}
		raw = string(encoded)
	}
	_, e := s.Core.DB.ExecContext(ctx, `UPDATE quote_batch_items SET item_status=?,error_code=?,result_json=?,updated_at=UTC_TIMESTAMP(6) WHERE batch_id=? AND position=? AND item_status='pending'`, status, errorCode, raw, batch, position)
	return e
}

func (s *Service) executeBulkItem(ctx context.Context, a core.Actor, batch, action, target string, item bulkPendingItem) (any, error) {
	key := fmt.Sprintf("quote-bulk-%s-%s-%d", action, batch, item.Position)
	switch action {
	case "status":
		return s.Transition(ctx, a, item.ID, key, TransitionInput{Status: target, Version: item.Version})
	case "clone":
		return s.Clone(ctx, a, item.ID, key, item.Version)
	case "export":
		q, e := s.Get(ctx, a, item.ID)
		if e != nil {
			return nil, e
		}
		if q.Deleted != nil || q.Version != item.Version {
			return nil, core.Conflict
		}
		return Summary{ID: q.ID, Number: q.Number, Name: q.Customer.Name, Reference: q.Reference, Date: q.Date, Currency: q.Currency, Total: q.Calculation.Total, Status: q.Status, Version: q.Version}, nil
	default:
		return nil, core.Invalid
	}
}

func (s *Service) finishBulkBatch(ctx context.Context, a core.Actor, batch, action string) error {
	return s.Core.Transact(ctx, func(tx *sql.Tx) error {
		var status, owner, storedAction string
		if e := tx.QueryRowContext(ctx, `SELECT status,CAST(requested_by AS CHAR),action FROM quote_batches WHERE id=? FOR UPDATE`, batch).Scan(&status, &owner, &storedAction); e != nil {
			if e == sql.ErrNoRows {
				return core.Missing
			}
			return e
		}
		if owner != a.UserID || storedAction != action {
			return core.Missing
		}
		if status == "completed" {
			return nil
		}
		if status != "processing" {
			return core.Conflict
		}
		var pending int
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM quote_batch_items WHERE batch_id=? AND item_status='pending'`, batch).Scan(&pending); e != nil {
			return e
		}
		if pending != 0 {
			return nil
		}
		if e := core.Changed(tx.ExecContext(ctx, `UPDATE quote_batches SET status='completed',completed_at=UTC_TIMESTAMP(6) WHERE id=? AND status='processing'`, batch)); e != nil {
			return e
		}
		return s.Core.Audit(ctx, tx, a, "quote.bulk_completed", "quote_batches", batch, "success", map[string]string{"action": action})
	})
}

func csvSafe(v string) string {
	trimmed := strings.TrimLeft(v, " \t\r\n")
	if len(trimmed) > 0 && strings.ContainsRune("=+-@", rune(trimmed[0])) {
		return "'" + v
	}
	return v
}

func bulkCSV(results []BulkItemResult) (string, error) {
	var b bytes.Buffer
	w := csv.NewWriter(&b)
	if e := w.Write([]string{"Número", "Cliente", "Referencia", "Fecha", "Moneda", "Total", "Estado", "Versión"}); e != nil {
		return "", e
	}
	for _, item := range results {
		if item.Status != "success" || len(item.Result) == 0 {
			continue
		}
		var row Summary
		if e := json.Unmarshal(item.Result, &row); e != nil {
			return "", e
		}
		values := []string{row.Number, row.Name, row.Reference, row.Date, row.Currency, row.Total, row.Status, fmt.Sprint(row.Version)}
		for i := range values {
			values[i] = csvSafe(values[i])
		}
		if e := w.Write(values); e != nil {
			return "", e
		}
	}
	w.Flush()
	if e := w.Error(); e != nil {
		return "", e
	}
	return "\ufeff" + b.String(), nil
}

func (s *Service) loadBulkResponse(ctx context.Context, a core.Actor, batch, action string) (BulkResponse, error) {
	out := BulkResponse{BatchID: batch, Action: action, Results: []BulkItemResult{}}
	var owner, storedAction string
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT CAST(requested_by AS CHAR),action,status FROM quote_batches WHERE id=?`, batch).Scan(&owner, &storedAction, &out.Status); e != nil {
		if e == sql.ErrNoRows {
			return out, core.Missing
		}
		return out, e
	}
	if owner != a.UserID || storedAction != action {
		return out, core.Missing
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT position,CAST(quote_id AS CHAR),quote_version,item_status,error_code,result_json FROM quote_batch_items WHERE batch_id=? ORDER BY position`, batch)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		var item BulkItemResult
		var result sql.NullString
		if e = rows.Scan(&item.Position, &item.ID, &item.Version, &item.Status, &item.Error, &result); e != nil {
			return out, e
		}
		if result.Valid {
			item.Result = json.RawMessage(append([]byte(nil), []byte(result.String)...))
		}
		out.Results = append(out.Results, item)
	}
	if e = rows.Err(); e != nil {
		return out, e
	}
	if action == "export" && out.Status == "completed" {
		out.Filename = "cotizaciones-" + batch + ".csv"
		out.Content, e = bulkCSV(out.Results)
		if e != nil {
			return out, e
		}
	}
	return out, nil
}

func (s *Service) runBulkBatch(ctx context.Context, a core.Actor, batch, action string) (BulkResponse, error) {
	var owner, storedAction, target, status string
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT CAST(requested_by AS CHAR),action,target_status,status FROM quote_batches WHERE id=?`, batch).Scan(&owner, &storedAction, &target, &status); e != nil {
		if e == sql.ErrNoRows {
			return BulkResponse{}, core.Missing
		}
		return BulkResponse{}, e
	}
	if owner != a.UserID || storedAction != action {
		return BulkResponse{}, core.Missing
	}
	if status != "processing" && status != "completed" {
		return BulkResponse{}, core.Conflict
	}
	if status == "processing" {
		items, e := s.pendingBulkItems(ctx, batch)
		if e != nil {
			return BulkResponse{}, e
		}
		for _, item := range items {
			result, e := s.executeBulkItem(ctx, a, batch, action, target, item)
			if e != nil {
				if !core.IsClient(e) {
					return BulkResponse{}, e
				}
				if e = s.settleBulkItem(ctx, batch, item.Position, "failed", bulkErrorCode(e), nil); e != nil {
					return BulkResponse{}, e
				}
				continue
			}
			if e = s.settleBulkItem(ctx, batch, item.Position, "success", "", result); e != nil {
				return BulkResponse{}, e
			}
		}
		if e := s.finishBulkBatch(ctx, a, batch, action); e != nil {
			return BulkResponse{}, e
		}
	}
	return s.loadBulkResponse(ctx, a, batch, action)
}

func (s *Service) BulkStatus(ctx context.Context, a core.Actor, key string, p BulkStatusInput) (BulkResponse, error) {
	if !editableStatus(p.Status) {
		return BulkResponse{}, core.Invalid
	}
	batch, e := s.createBulkBatch(ctx, a, key, "status", p.Status, p.Items)
	if e != nil {
		return BulkResponse{}, e
	}
	return s.runBulkBatch(ctx, a, batch, "status")
}

func (s *Service) BulkClone(ctx context.Context, a core.Actor, key string, p BulkCloneInput) (BulkResponse, error) {
	batch, e := s.createBulkBatch(ctx, a, key, "clone", "", p.Items)
	if e != nil {
		return BulkResponse{}, e
	}
	return s.runBulkBatch(ctx, a, batch, "clone")
}

func (s *Service) BulkExport(ctx context.Context, a core.Actor, key string, p BulkExportInput) (BulkResponse, error) {
	batch, e := s.createBulkBatch(ctx, a, key, "export", "", p.Items)
	if e != nil {
		return BulkResponse{}, e
	}
	return s.runBulkBatch(ctx, a, batch, "export")
}
