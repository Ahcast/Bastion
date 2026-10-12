package invoices

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
	"time"
)

type PaymentInput struct {
	Version   uint64 `json:"version,string"`
	Date      string `json:"date"`
	Method    string `json:"method"`
	Currency  string `json:"currency"`
	Amount    string `json:"amount"`
	Reference string `json:"reference"`
	Reason    string `json:"reason"`
}
type VoidPaymentInput struct {
	Version        uint64 `json:"version,string"`
	PaymentVersion uint64 `json:"payment_version,string"`
	Reason         string `json:"reason"`
}
type Payment struct {
	ID            string     `json:"id"`
	InvoiceID     string     `json:"invoice_id"`
	Date          string     `json:"date"`
	Method        string     `json:"method"`
	Currency      string     `json:"currency"`
	Amount        string     `json:"amount"`
	Reference     string     `json:"reference"`
	Reason        string     `json:"reason"`
	ReceiptNumber *string    `json:"receipt_number"`
	Version       uint64     `json:"version,string"`
	CreatedBy     string     `json:"created_by"`
	CreatedAt     time.Time  `json:"created_at"`
	VoidedAt      *time.Time `json:"voided_at"`
	VoidedBy      *string    `json:"voided_by"`
	VoidReason    *string    `json:"void_reason"`
}
type PaymentResult struct {
	InvoiceID      string `json:"invoice_id"`
	PaymentID      string `json:"payment_id"`
	Version        uint64 `json:"version,string"`
	PaymentVersion uint64 `json:"payment_version,string"`
	PaymentProjection
	CommercialStatus string `json:"commercial_status"`
}
type PaymentPage struct {
	SourceOrderClosed *bool     `json:"source_order_closed"`
	Data              []Payment `json:"data"`
	Total             int64     `json:"total"`
	Page              int       `json:"page"`
	PerPage           int       `json:"per_page"`
	Version           uint64    `json:"version,string"`
	PaymentProjection
	CommercialStatus *string `json:"commercial_status"`
}

// CommercialStatus derives collection state without rewriting document status.
func CommercialStatus(status string, due *string, p PaymentProjection, today string) (*string, error) {
	if status == "draft" {
		return nil, nil
	}
	if !validCurrency(p.Currency) {
		return nil, core.Invalid
	}
	if status != "issued" || due == nil || !fiscalDate(*due) || !fiscalDate(today) {
		return nil, core.Conflict
	}
	paid, e := finance.ParseDecimal(p.PaidTotal, 2)
	if e != nil {
		return nil, e
	}
	balance, e := finance.ParseDecimal(p.Balance, 2)
	if e != nil {
		return nil, e
	}
	result := "pending"
	if balance.IsZero() {
		result = "paid"
	} else if *due < today {
		result = "overdue"
	} else if paid.GreaterThan(decimal.Zero) {
		result = "partially_paid"
	}
	return &result, nil
}
func invoiceCommercial(o *Invoice, now time.Time) error {
	today, e := invoiceBusinessDate(now)
	if e != nil {
		return e
	}
	o.CommercialStatus, e = CommercialStatus(o.Status, o.DueDate, PaymentProjection{Currency: o.Currency, PaidTotal: o.PaidTotal, Balance: o.Balance}, today)
	return e
}
func (s *Service) paymentInvoice(ctx context.Context, tx *sql.Tx, a core.Actor, id, permission string, modes ...string) (Invoice, error) {
	mode := "read"
	if len(modes) > 0 {
		mode = modes[0]
	}
	o, _, e := s.lockSourceInvoice(ctx, tx, a, id, permission, mode)
	return o, e
}

func (s *Service) paymentProjection(ctx context.Context, tx *sql.Tx, o Invoice, exclude string) (PaymentProjection, error) {
	rows, e := tx.QueryContext(ctx, `SELECT CAST(id AS CHAR),currency,CAST(amount AS CHAR) FROM payments WHERE invoice_id=? AND voided_at IS NULL ORDER BY id`, o.ID)
	if e != nil {
		return PaymentProjection{}, e
	}
	defer rows.Close()
	out, e := ProjectPayments(o.Currency, o.Calculation.Total, nil)
	if e != nil {
		return out, e
	}
	for rows.Next() {
		var id, currency, amount string
		if e = rows.Scan(&id, &currency, &amount); e != nil {
			return out, e
		}
		if currency != o.Currency {
			return out, core.Conflict
		}
		if id != exclude {
			out, e = ApplyPayment(o.Currency, o.Calculation.Total, out.PaidTotal, amount)
			if e != nil {
				return out, core.Conflict
			}
		}
	}
	return out, rows.Err()
}
func (s *Service) checkedPaymentProjection(ctx context.Context, tx *sql.Tx, o Invoice) (PaymentProjection, error) {
	p, e := s.paymentProjection(ctx, tx, o, "")
	if e != nil {
		return p, e
	}
	if p.PaidTotal != o.PaidTotal || p.Balance != o.Balance {
		return p, core.Conflict
	}
	return p, nil
}

const paymentColumns = `CAST(id AS CHAR),CAST(invoice_id AS CHAR),CAST(payment_date AS CHAR),method,currency,CAST(amount AS CHAR),reference,reason,receipt_number,version,CAST(created_by AS CHAR),created_at,voided_at,CAST(voided_by AS CHAR),void_reason`

func paymentTargets(p *Payment) []any {
	return []any{&p.ID, &p.InvoiceID, &p.Date, &p.Method, &p.Currency, &p.Amount, &p.Reference, &p.Reason, &p.ReceiptNumber, &p.Version, &p.CreatedBy, &p.CreatedAt, &p.VoidedAt, &p.VoidedBy, &p.VoidReason}
}
func (s *Service) Payments(ctx context.Context, a core.Actor, id string, page, perPage int) (PaymentPage, error) {
	out := PaymentPage{Data: []Payment{}, Page: page, PerPage: perPage}
	if core.ID(id) != nil || page < 1 || page > 100000 || perPage < 1 || perPage > 100 {
		return out, core.Invalid
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "facturas.view"); e != nil {
			return e
		}
		o, e := s.paymentInvoice(ctx, tx, a, id, "facturas.view")
		if e != nil {
			return e
		}
		p, e := s.checkedPaymentProjection(ctx, tx, o)
		if e != nil {
			return e
		}
		out.SourceOrderClosed = o.SourceOrderClosed
		out.Version = o.Version
		out.PaymentProjection = p
		if e = invoiceCommercial(&o, time.Now().UTC()); e != nil {
			return e
		}
		out.CommercialStatus = o.CommercialStatus
		if e = tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM payments WHERE invoice_id=?`, id).Scan(&out.Total); e != nil {
			return e
		}
		rows, e := tx.QueryContext(ctx, `SELECT `+paymentColumns+` FROM payments WHERE invoice_id=? ORDER BY payment_date DESC,id DESC LIMIT ? OFFSET ?`, id, perPage, (page-1)*perPage)
		if e != nil {
			return e
		}
		defer rows.Close()
		for rows.Next() {
			var p Payment
			if e = rows.Scan(paymentTargets(&p)...); e != nil {
				return e
			}
			out.Data = append(out.Data, p)
		}
		return rows.Err()
	})
	return out, e
}
func validPaymentMethod(m string) bool {
	return m == "Siafi" || m == "Cheque" || m == "Transferencia" || m == "Efectivo"
}
func (s *Service) RegisterPayment(ctx context.Context, a core.Actor, id, key string, p PaymentInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	amount, e := finance.ParseDecimal(p.Amount, 2)
	if core.ID(id) != nil || key == "" || p.Version == 0 || !fiscalDate(p.Date) || !validPaymentMethod(p.Method) || !validCurrency(p.Currency) || e != nil || !amount.GreaterThan(decimal.Zero) || !finance.ValidText(p.Reference, 500, false) || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	return s.writePayment(ctx, a, id, "", key, "facturas.payment", p.Version, 0, p.Reason, p, func(tx *sql.Tx, o Invoice, now time.Time, current PaymentProjection) (string, uint64, PaymentProjection, error) {
		today, e := invoiceBusinessDate(now)
		if e != nil {
			return "", 0, current, e
		}
		if p.Date < o.InvoiceDate || p.Date > today {
			return "", 0, current, core.Invalid
		}
		if p.Currency != o.Currency {
			return "", 0, current, core.Conflict
		}
		next, e := ApplyPayment(o.Currency, o.Calculation.Total, current.PaidTotal, p.Amount)
		if e != nil {
			return "", 0, current, e
		}
		r, e := tx.ExecContext(ctx, `INSERT INTO payments(invoice_id,payment_date,method,currency,amount,reference,reason,version,created_by,created_at) VALUES(?,?,?,?,?,?,?,1,?,?)`, id, p.Date, p.Method, p.Currency, amount.StringFixed(2), p.Reference, p.Reason, a.UserID, now)
		if e != nil {
			return "", 0, current, e
		}
		n, e := r.LastInsertId()
		return fmt.Sprint(n), 1, next, e
	})
}
func (s *Service) VoidPayment(ctx context.Context, a core.Actor, id, paymentID, key string, p VoidPaymentInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || core.ID(paymentID) != nil || key == "" || p.Version == 0 || p.PaymentVersion == 0 || !finance.ValidText(p.Reason, 1000, true) {
		return nil, core.Invalid
	}
	return s.writePayment(ctx, a, id, paymentID, key, "facturas.payment.void", p.Version, p.PaymentVersion, p.Reason, p, func(tx *sql.Tx, o Invoice, now time.Time, current PaymentProjection) (string, uint64, PaymentProjection, error) {
		var payment Payment
		e := tx.QueryRowContext(ctx, `SELECT `+paymentColumns+` FROM payments WHERE invoice_id=? AND id=? FOR UPDATE`, id, paymentID).Scan(paymentTargets(&payment)...)
		if e == sql.ErrNoRows {
			return "", 0, current, core.Missing
		}
		if e != nil {
			return "", 0, current, e
		}
		if payment.VoidedAt != nil || payment.Version != p.PaymentVersion || payment.Version == ^uint64(0) {
			return "", 0, current, core.Conflict
		}
		next, e := s.paymentProjection(ctx, tx, o, paymentID)
		if e != nil {
			return "", 0, current, e
		}
		e = core.Changed(tx.ExecContext(ctx, `UPDATE payments SET voided_at=?,voided_by=?,void_reason=?,version=version+1 WHERE invoice_id=? AND id=? AND version=? AND voided_at IS NULL`, now, a.UserID, p.Reason, id, paymentID, p.PaymentVersion))
		return paymentID, payment.Version + 1, next, e
	})
}

type paymentMutation func(*sql.Tx, Invoice, time.Time, PaymentProjection) (string, uint64, PaymentProjection, error)

func (s *Service) writePayment(ctx context.Context, a core.Actor, id, paymentID, key, permission string, version, paymentVersion uint64, reason string, payload any, mutate paymentMutation) (any, error) {
	var out any
	topic := "payment.registered"
	if permission == "facturas.payment.void" {
		topic = "payment.voided"
	}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gateSensitive(ctx, tx, a, permission, true); e != nil {
			return e
		}
		var e error
		out, e = s.Core.Idempotent(ctx, tx, a, permission+":"+id+":"+paymentID, key, payload, func() (any, error) {
			o, e := s.paymentInvoice(ctx, tx, a, id, permission, "payment")
			if e != nil {
				return nil, e
			}
			if o.Status != "issued" || o.Number == nil || o.DueDate == nil || o.Version != version || o.Version == ^uint64(0) {
				return nil, core.Conflict
			}
			current, e := s.checkedPaymentProjection(ctx, tx, o)
			if e != nil {
				return nil, e
			}
			now := time.Now().UTC().Truncate(time.Microsecond)
			pid, pv, next, e := mutate(tx, o, now, current)
			if e != nil {
				return nil, e
			}
			o.PaidTotal = next.PaidTotal
			o.Balance = next.Balance
			if e = invoiceCommercial(&o, now); e != nil {
				return nil, e
			}
			if e = core.Changed(tx.ExecContext(ctx, `UPDATE invoices SET paid_total=?,balance=?,version=version+1,updated_by=?,updated_at=? WHERE id=? AND version=? AND status='issued'`, next.PaidTotal, next.Balance, a.UserID, now, id, version)); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, topic, "payments", pid, "success", map[string]string{"invoice_id": id, "version": fmt.Sprint(version + 1), "payment_version": fmt.Sprint(pv), "reason": reason}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, topic, "payments", pid, pv); e != nil {
				return nil, e
			}
			return PaymentResult{InvoiceID: id, PaymentID: pid, Version: version + 1, PaymentVersion: pv, PaymentProjection: next, CommercialStatus: *o.CommercialStatus}, nil
		})
		if e != nil {
			return e
		}
		_, e = s.paymentInvoice(ctx, tx, a, id, permission)
		return e
	})
	return out, dbError(e)
}
