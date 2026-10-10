package processes

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/csv"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"strings"
	"time"
)

func (s *Service) Export(ctx context.Context, a core.Actor, f Filter) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	f.export = true
	f.Page = 1
	f.PerPage = 100
	var b bytes.Buffer
	w := csv.NewWriter(&b)
	_ = w.Write([]string{"Entidad", "Expediente", "Fecha recepción ofertas", "Especificaciones"})
	for {
		list, e := s.List(ctx, a, f)
		if e != nil {
			return nil, e
		}
		if list.Total > 10000 {
			return nil, &core.Error{Code: "export_limit", Message: "Aplique filtros para exportar un máximo de 10000 procesos.", Status: 422}
		}
		for _, p := range list.Data {
			values := []string{p.Entity, p.Reference, p.Deadline, p.Description}
			for i, v := range values {
				v = strings.TrimLeft(v, " \t\r\n")
				if len(v) > 0 && strings.ContainsRune("=+-@", rune(v[0])) {
					values[i] = "'" + values[i]
				}
			}
			if e = w.Write(values); e != nil {
				return nil, e
			}
			w.Flush()
			if b.Len() > 8*1024*1024 {
				return nil, &core.Error{Code: "export_limit", Message: "La exportación excede 8 MiB. Aplique filtros.", Status: 422}
			}
		}
		if int64(f.Page*f.PerPage) >= list.Total {
			break
		}
		f.Page++
	}
	w.Flush()
	if e := w.Error(); e != nil {
		return nil, e
	}
	if e := s.Core.Audit(ctx, s.Core.DB, a, "processes.export", "processes", "", "success", map[string]string{}); e != nil {
		return nil, e
	}
	return map[string]string{"filename": "procesos.csv", "content": "\ufeff" + b.String()}, nil
}
func (s *Service) Status(ctx context.Context, a core.Actor) (any, error) {
	if e := s.Core.Authorize(ctx, s.Core.DB, a, "procesos.admin", "", "", false); e != nil {
		return nil, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT r.id,r.started_at,d.batch_id,d.payload,d.created_at FROM crawler_runs r LEFT JOIN crawler_deliveries d ON d.run_id=r.id AND d.kind='complete' ORDER BY COALESCE(d.created_at,r.created_at) DESC,r.id DESC LIMIT 20`)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []map[string]any{}
	for rows.Next() {
		var id string
		var at time.Time
		var batch, payload sql.NullString
		var finished sql.NullTime
		if e = rows.Scan(&id, &at, &batch, &payload, &finished); e != nil {
			return nil, e
		}
		v := map[string]any{"run_id": id, "started_at": at, "status": "PENDING_DELIVERY"}
		if payload.Valid {
			var p Complete
			if json.Unmarshal([]byte(payload.String), &p) != nil {
				return nil, core.Conflict
			}
			var status string
			_ = json.Unmarshal(p.Extraction["status"], &status)
			v["status"] = status
			v["import"] = p.Import
			v["batch_id"] = batch.String
			v["received_at"] = finished.Time
		}
		out = append(out, v)
	}
	return map[string]any{"data": out, "limit": 20}, rows.Err()
}
