package reports

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/shopspring/decimal"
	"strings"
	"time"
)

const MaxExportRows = 10000
const MaxExportXML = 24 * 1024 * 1024

var exportSlots = make(chan struct{}, 2)
var exportTooLarge = &core.Error{Code: "export_too_large", Message: "La exportación supera el límite. Reduzca el período o filtre una moneda.", Status: 422}

type Export struct {
	Data        []byte
	GeneratedAt time.Time
}

func exportSafety(ctx context.Context, q core.Querier, p projection, w string, args []any) (map[string]map[string]bool, error) {
	cols := []string{"r.currency"}
	moneyCols := []column{}
	for _, c := range p.columns {
		if c.money {
			moneyCols = append(moneyCols, c)
			cols = append(cols, "CAST(SUM(ABS("+c.expression+")*100) AS CHAR)")
		}
	}
	rows, e := q.QueryContext(ctx, "SELECT "+strings.Join(cols, ",")+" FROM "+p.table+" r WHERE "+w+" GROUP BY r.currency", args...)
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := map[string]map[string]bool{}
	for rows.Next() {
		var currency string
		values := make([]string, len(moneyCols))
		dest := []any{&currency}
		for i := range values {
			dest = append(dest, &values[i])
		}
		if e = rows.Scan(dest...); e != nil {
			return nil, e
		}
		out[currency] = map[string]bool{}
		for i, c := range moneyCols {
			d, e := decimal.NewFromString(values[i])
			if e != nil {
				return nil, e
			}
			out[currency][c.name] = d.LessThanOrEqual(decimal.NewFromInt(999999999999999))
		}
	}
	return out, rows.Err()
}
func (s *Service) ExportOrders(ctx context.Context, a core.Actor, f Filter) (Export, error) {
	out := Export{}
	if a.Type != "user" {
		return out, core.Forbidden
	}
	f.Page = 1
	f.PerPage = 25
	if e := validate("orders", f); e != nil {
		return out, e
	}
	e := s.snapshot(ctx, a, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "reportes.export", true); e != nil {
			return e
		}
		p, w, args, e := s.where(ctx, tx, a, "orders", f, true)
		if e != nil {
			return e
		}
		select {
		case exportSlots <- struct{}{}:
			defer func() { <-exportSlots }()
		default:
			return &core.Error{Code: "export_busy", Message: "Hay exportaciones en curso. Intente de nuevo en unos momentos.", Status: 429}
		}
		ts, e := totals(ctx, tx, p, w, args)
		if e != nil {
			return e
		}
		var count int64
		for _, t := range ts {
			count += t.Count
		}
		if count > MaxExportRows {
			return exportTooLarge
		}
		// Reject definitely oversized UTF-8 payloads before allocating the row set.
		var textBytes int64
		if e = tx.QueryRowContext(ctx, "SELECT COALESCE(SUM(OCTET_LENGTH(r.order_number)+OCTET_LENGTH(r.customer_name_snapshot)+OCTET_LENGTH(r.destination)),0) FROM "+p.table+" r WHERE "+w, args...).Scan(&textBytes); e != nil {
			return e
		}
		if textBytes > MaxExportXML {
			return exportTooLarge
		}

		safety, e := exportSafety(ctx, tx, p, w, args)
		if e != nil {
			return e
		}
		rows, e := readRows(ctx, tx, p, w, args, MaxExportRows, 0)
		if e != nil {
			return e
		}
		if int64(len(rows)) != count {
			return core.Conflict
		}
		out.GeneratedAt = time.Now().UTC()
		out.Data, e = workbook(ctx, f, rows, ts, safety, out.GeneratedAt)
		if e != nil {
			return e
		}
		if e = ctx.Err(); e != nil {
			return e
		}
		return s.Core.Audit(ctx, tx, a, "reports.orders.export", "reports", "orders", "success", map[string]any{"filters": f, "rows": count, "generated_at": out.GeneratedAt.Format(time.RFC3339Nano)})
	})
	if e != nil {
		return Export{}, e
	}
	return out, nil
}
