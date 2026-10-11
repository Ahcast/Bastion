package invoices

import (
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"math"
	"strings"
	"time"
	"unicode"
)

// FiscalAuthorization carries configured values, not a claim of SAR validation.
type FiscalAuthorization struct {
	CAI        string `json:"cai"`
	Prefix     string `json:"prefix"`
	Width      uint8  `json:"number_width"`
	RangeStart uint64 `json:"range_start,string"`
	RangeEnd   uint64 `json:"range_end,string"`
	ValidFrom  string `json:"valid_from"`
	ValidUntil string `json:"valid_until"`
	Currency   string `json:"currency"`
	Enabled    bool   `json:"enabled"`
}
type FiscalAllocation struct {
	Number    string              `json:"number"`
	NextValue uint64              `json:"next_value,string"`
	Snapshot  FiscalAuthorization `json:"snapshot"`
}

func fiscalDate(v string) bool {
	d, e := time.Parse("2006-01-02", v)
	return e == nil && d.Year() >= 1900 && d.Year() <= 9999 && d.Format("2006-01-02") == v
}
func ValidateFiscalAuthorization(p FiscalAuthorization) error {
	if !finance.ValidText(p.CAI, 80, true) || len(p.CAI) > 80 || strings.IndexFunc(p.CAI, unicode.IsControl) >= 0 || len(p.Prefix) > 60 || p.Width < 1 || p.Width > 20 || p.RangeStart == 0 || p.RangeEnd < p.RangeStart || p.RangeEnd == math.MaxUint64 || !validCurrency(p.Currency) || !fiscalDate(p.ValidFrom) || !fiscalDate(p.ValidUntil) || p.ValidFrom > p.ValidUntil {
		return core.Invalid
	}
	for _, c := range p.Prefix {
		if c < 33 || c > 126 {
			return core.Invalid
		}
	}
	if len(fmt.Sprint(p.RangeEnd)) > int(p.Width) || len(p.Prefix)+int(p.Width) > 80 {
		return core.Invalid
	}
	return nil
}

// AllocateFiscalNumber only projects an allocation. The caller must lock and
// persist the counter and immutable invoice evidence in one authorized TX.
func AllocateFiscalNumber(p FiscalAuthorization, next uint64, invoiceDate, businessDate, currency string) (FiscalAllocation, error) {
	var zero FiscalAllocation
	if e := ValidateFiscalAuthorization(p); e != nil {
		return zero, e
	}
	if !fiscalDate(invoiceDate) || !fiscalDate(businessDate) || !validCurrency(currency) {
		return zero, core.Invalid
	}
	if !p.Enabled || currency != p.Currency || next < p.RangeStart || next > p.RangeEnd || businessDate < p.ValidFrom || businessDate > p.ValidUntil || invoiceDate < p.ValidFrom || invoiceDate > p.ValidUntil || invoiceDate > businessDate {
		return zero, core.Conflict
	}
	return FiscalAllocation{Number: p.Prefix + fmt.Sprintf("%0*d", int(p.Width), next), NextValue: next + 1, Snapshot: p}, nil
}
