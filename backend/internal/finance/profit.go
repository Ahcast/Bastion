package finance

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/shopspring/decimal"
)

// Profit is net sales after discounts, excluding tax, minus total investment.
// Inputs are already monetary amounts; no alternate rounding or currency mixing.
func Profit(base15, base18, exempt, exonerated, investment string) (string, error) {
	net := decimal.Zero
	for _, value := range []string{base15, base18, exempt, exonerated} {
		d, e := ParseDecimal(value, 2)
		if e != nil {
			return "", e
		}
		net = net.Add(d)
	}
	cost, e := ParseDecimal(investment, 2)
	if e != nil {
		return "", e
	}
	profit := net.Sub(cost)
	if net.GreaterThan(moneyLimit) || cost.GreaterThan(moneyLimit) || profit.Abs().GreaterThan(moneyLimit) {
		return "", core.Invalid
	}
	return profit.StringFixed(2), nil
}
