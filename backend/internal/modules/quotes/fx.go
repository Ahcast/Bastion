package quotes

import (
	"context"
	"database/sql"
	"regexp"
	"strings"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/shopspring/decimal"
)

func init() {
	Permissions = append(Permissions, "cotizaciones.fx.view", "cotizaciones.fx.manage")
}

var fxHashPattern = regexp.MustCompile(`^[a-f0-9]{64}$`)

type FXObservationInput struct {
	RateKind          string `json:"rate_kind"`
	Rate              string `json:"rate"`
	ValidFrom         string `json:"valid_from"`
	ValidTo           string `json:"valid_to"`
	Source            string `json:"source"`
	SourceReference   string `json:"source_reference"`
	SourcePublishedAt string `json:"source_published_at"`
	EvidenceHash      string `json:"evidence_hash"`
	SupersedesID      string `json:"supersedes_id"`
	Reason            string `json:"reason"`
}

type FXObservation struct {
	ID                string     `json:"id"`
	BaseCurrency      string     `json:"base_currency"`
	QuoteCurrency     string     `json:"quote_currency"`
	RateKind          string     `json:"rate_kind"`
	Rate              string     `json:"rate"`
	ValidFrom         time.Time  `json:"valid_from"`
	ValidTo           *time.Time `json:"valid_to"`
	Source            string     `json:"source"`
	SourceReference   string     `json:"source_reference"`
	SourcePublishedAt *time.Time `json:"source_published_at"`
	EvidenceHash      string     `json:"evidence_hash"`
	SupersedesID      string     `json:"supersedes_id"`
	Superseded        bool       `json:"superseded"`
	Reason            string     `json:"reason"`
	CreatedBy         string     `json:"created_by"`
	CreatedByName     string     `json:"created_by_name"`
	CreatedAt         time.Time  `json:"created_at"`
}

type FXFilter struct {
	RateKind string
	Source   string
	Page     int
	PerPage  int
}

type FXPage struct {
	Data    []FXObservation `json:"data"`
	Total   int64           `json:"total"`
	Page    int             `json:"page"`
	PerPage int             `json:"per_page"`
}

func validFXKind(v string) bool {
	switch v {
	case "tcr", "buy", "sell", "manual":
		return true
	default:
		return false
	}
}

func validFXSource(v string) bool {
	return v == "bch" || v == "manual"
}

func normalizeFXRate(v string) (string, error) {
	v = strings.TrimSpace(v)
	if len(v) == 0 || len(v) > 25 || !decimalPattern.MatchString(v) {
		return "", core.Invalid
	}
	parts := strings.Split(v, ".")
	if len(parts[0]) > 12 || (len(parts) == 2 && len(parts[1]) > 12) {
		return "", core.Invalid
	}
	d, e := decimal.NewFromString(v)
	if e != nil || !d.IsPositive() {
		return "", core.Invalid
	}
	return d.String(), nil
}

func parseFXTime(v string, required bool) (*time.Time, error) {
	v = strings.TrimSpace(v)
	if v == "" {
		if required {
			return nil, core.Invalid
		}
		return nil, nil
	}
	t, e := time.Parse(time.RFC3339Nano, v)
	if e != nil || t.Year() < 1970 || t.Year() > 9999 {
		return nil, core.Invalid
	}
	t = t.UTC().Truncate(time.Microsecond)
	return &t, nil
}

func normalizeFXInput(p *FXObservationInput) (*time.Time, *time.Time, *time.Time, error) {
	p.RateKind = strings.ToLower(strings.TrimSpace(p.RateKind))
	p.Source = strings.ToLower(strings.TrimSpace(p.Source))
	p.SourceReference = strings.TrimSpace(p.SourceReference)
	p.EvidenceHash = strings.TrimSpace(p.EvidenceHash)
	p.SupersedesID = strings.TrimSpace(p.SupersedesID)
	p.Reason = strings.TrimSpace(p.Reason)
	if !validFXKind(p.RateKind) || !validFXSource(p.Source) || !validText(p.SourceReference, 1000, true) || !validText(p.Reason, 1000, false) {
		return nil, nil, nil, core.Invalid
	}
	if p.SupersedesID != "" && core.ID(p.SupersedesID) != nil {
		return nil, nil, nil, core.Invalid
	}
	if p.EvidenceHash != "" && !fxHashPattern.MatchString(p.EvidenceHash) {
		return nil, nil, nil, core.Invalid
	}
	if (p.Source == "manual" || p.RateKind == "manual" || p.SupersedesID != "") && p.Reason == "" {
		return nil, nil, nil, core.Invalid
	}
	var e error
	if p.Rate, e = normalizeFXRate(p.Rate); e != nil {
		return nil, nil, nil, e
	}
	from, e := parseFXTime(p.ValidFrom, true)
	if e != nil {
		return nil, nil, nil, e
	}
	to, e := parseFXTime(p.ValidTo, false)
	if e != nil {
		return nil, nil, nil, e
	}
	published, e := parseFXTime(p.SourcePublishedAt, false)
	if e != nil {
		return nil, nil, nil, e
	}
	if to != nil && !to.After(*from) {
		return nil, nil, nil, core.Invalid
	}
	p.ValidFrom = from.Format(time.RFC3339Nano)
	if to != nil {
		p.ValidTo = to.Format(time.RFC3339Nano)
	} else {
		p.ValidTo = ""
	}
	if published != nil {
		p.SourcePublishedAt = published.Format(time.RFC3339Nano)
	} else {
		p.SourcePublishedAt = ""
	}
	return from, to, published, nil
}

func nullableTime(v *time.Time) any {
	if v == nil {
		return nil
	}
	return *v
}

// FX observations are global reference data, not records owned by one user/team.
// Requiring scope=all avoids accidentally granting global rate visibility through
// a self/team grant. Managing the ledger is sensitive and requires recent reauth.
func (s *Service) fxGate(ctx context.Context, q core.Querier, a core.Actor, permission string, sensitive bool) error {
	return s.Core.Authorize(ctx, q, a, permission, "", "", sensitive)
}

func (s *Service) RecordFXObservation(ctx context.Context, a core.Actor, key string, p FXObservationInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	from, to, published, e := normalizeFXInput(&p)
	if e != nil {
		return nil, e
	}
	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.fxGate(ctx, tx, a, "cotizaciones.fx.manage", true); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, "quotes.fx_observation", key, p, func() (any, error) {
			if p.SupersedesID != "" {
				var original string
				if e := tx.QueryRowContext(ctx, `SELECT CAST(id AS CHAR) FROM quote_fx_observations WHERE id=? FOR UPDATE`, p.SupersedesID).Scan(&original); e == sql.ErrNoRows {
					return nil, core.Missing
				} else if e != nil {
					return nil, e
				}
				var n int
				if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM quote_fx_observations WHERE supersedes_id=?`, p.SupersedesID).Scan(&n); e != nil {
					return nil, e
				} else if n != 0 {
					return nil, core.Conflict
				}
			}
			res, e := tx.ExecContext(ctx, `INSERT INTO quote_fx_observations(base_currency,quote_currency,rate_kind,rate,valid_from,valid_to,source,source_reference,source_published_at,evidence_hash,supersedes_id,reason,created_by,created_at) VALUES('USD','HNL',?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6))`, p.RateKind, p.Rate, *from, nullableTime(to), p.Source, p.SourceReference, nullableTime(published), p.EvidenceHash, nullable(p.SupersedesID), p.Reason, a.UserID)
			if e != nil {
				return nil, e
			}
			n, e := res.LastInsertId()
			if e != nil {
				return nil, e
			}
			id := core.WrapID(n)
			if e = s.Core.Audit(ctx, tx, a, "quotes.fx_observation_recorded", "quote_fx_observations", id, "success", map[string]string{"rate_kind": p.RateKind, "source": p.Source, "supersedes_id": p.SupersedesID}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "quote.fx_observation_recorded", "quote_fx_observations", id, 1); e != nil {
				return nil, e
			}
			return map[string]string{"id": id}, nil
		})
		return e
	})
	return out, e
}

const fxColumns = `o.id,o.base_currency,o.quote_currency,o.rate_kind,CAST(o.rate AS CHAR),o.valid_from,o.valid_to,o.source,o.source_reference,o.source_published_at,o.evidence_hash,COALESCE(CAST(o.supersedes_id AS CHAR),''),EXISTS(SELECT 1 FROM quote_fx_observations n WHERE n.supersedes_id=o.id),o.reason,CAST(o.created_by AS CHAR),u.name,o.created_at`

func scanFX(row interface{ Scan(...any) error }) (FXObservation, error) {
	var v FXObservation
	e := row.Scan(&v.ID, &v.BaseCurrency, &v.QuoteCurrency, &v.RateKind, &v.Rate, &v.ValidFrom, &v.ValidTo, &v.Source, &v.SourceReference, &v.SourcePublishedAt, &v.EvidenceHash, &v.SupersedesID, &v.Superseded, &v.Reason, &v.CreatedBy, &v.CreatedByName, &v.CreatedAt)
	if e == nil {
		v.Rate = decimal.RequireFromString(v.Rate).String()
	}
	return v, e
}

func (s *Service) GetFXObservation(ctx context.Context, a core.Actor, id string) (FXObservation, error) {
	var out FXObservation
	if core.ID(id) != nil {
		return out, core.Invalid
	}
	if e := s.fxGate(ctx, s.Core.DB, a, "cotizaciones.fx.view", false); e != nil {
		return out, e
	}
	out, e := scanFX(s.Core.DB.QueryRowContext(ctx, `SELECT `+fxColumns+` FROM quote_fx_observations o JOIN users u ON u.id=o.created_by WHERE o.id=?`, id))
	if e == sql.ErrNoRows {
		return FXObservation{}, core.Missing
	}
	return out, e
}

func (s *Service) ListFXObservations(ctx context.Context, a core.Actor, f FXFilter) (FXPage, error) {
	f.RateKind = strings.ToLower(strings.TrimSpace(f.RateKind))
	f.Source = strings.ToLower(strings.TrimSpace(f.Source))
	out := FXPage{Data: []FXObservation{}, Page: f.Page, PerPage: f.PerPage}
	if f.Page < 1 || f.Page > 100000 || f.PerPage < 1 || f.PerPage > 100 || (f.RateKind != "" && !validFXKind(f.RateKind)) || (f.Source != "" && !validFXSource(f.Source)) {
		return out, core.Invalid
	}
	if e := s.fxGate(ctx, s.Core.DB, a, "cotizaciones.fx.view", false); e != nil {
		return out, e
	}
	where := `1=1`
	args := []any{}
	if f.RateKind != "" {
		where += ` AND o.rate_kind=?`
		args = append(args, f.RateKind)
	}
	if f.Source != "" {
		where += ` AND o.source=?`
		args = append(args, f.Source)
	}
	if e := s.Core.DB.QueryRowContext(ctx, `SELECT COUNT(*) FROM quote_fx_observations o WHERE `+where, args...).Scan(&out.Total); e != nil {
		return out, e
	}
	rows, e := s.Core.DB.QueryContext(ctx, `SELECT `+fxColumns+` FROM quote_fx_observations o JOIN users u ON u.id=o.created_by WHERE `+where+` ORDER BY o.valid_from DESC,o.id DESC LIMIT ? OFFSET ?`, append(args, f.PerPage, (f.Page-1)*f.PerPage)...)
	if e != nil {
		return out, e
	}
	defer rows.Close()
	for rows.Next() {
		v, e := scanFX(rows)
		if e != nil {
			return out, e
		}
		out.Data = append(out.Data, v)
	}
	return out, rows.Err()
}
