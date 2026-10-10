package quotes

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/shopspring/decimal"
	"regexp"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

// All financial constructors start from validated strings. The allocation uses
// integer quotient/remainder, independent of Decimal.DivisionPrecision.
var decimalPattern = regexp.MustCompile(`^(0|[1-9][0-9]*)(\.[0-9]+)?$`)
var moneyLimit = decimal.RequireFromString("9999999999999999.99")

func number(v string, scale int32) (decimal.Decimal, error) {
	if len(v) > 24 || !decimalPattern.MatchString(v) {
		return decimal.Zero, core.Invalid
	}
	parts := strings.Split(v, ".")
	fraction := 0
	if len(parts) == 2 {
		fraction = len(parts[1])
	}
	if fraction > int(scale) || len(parts[0]) > 18-int(scale) {
		return decimal.Zero, core.Invalid
	}
	d, e := decimal.NewFromString(v)
	if e != nil {
		return decimal.Zero, core.Invalid
	}
	return d, nil
}
func validText(v string, n int, required bool) bool {
	return utf8.ValidString(v) && utf8.RuneCountInString(v) <= n && (!required || strings.TrimSpace(v) != "") && strings.IndexFunc(v, func(r rune) bool { return unicode.IsControl(r) && r != '\n' && r != '\t' }) < 0
}
func unitDiscount(price decimal.Decimal, kind, value string) (decimal.Decimal, error) {
	scale := int32(6)
	if kind == "percent" {
		scale = 4
	} else if kind != "amount" {
		return decimal.Zero, core.Invalid
	}
	d, e := number(value, scale)
	if e != nil {
		return d, e
	}
	if kind == "percent" {
		if d.GreaterThan(decimal.NewFromInt(100)) {
			return d, core.Invalid
		}
		d = price.Mul(d).Shift(-2)
	}
	if d.GreaterThan(price) {
		return d, core.Invalid
	}
	return d, nil
}

type Item struct {
	Description  string `json:"description"`
	Unit         string `json:"unit"`
	Quantity     string `json:"quantity"`
	Price        string `json:"unit_price"`
	Cost         string `json:"unit_cost"`
	DiscountType string `json:"discount_type"`
	Discount     string `json:"discount"`
	Tax          string `json:"tax"`
}
type CalculatedItem struct {
	Item
	UnitDiscount      string `json:"unit_discount"`
	Gross             string `json:"gross"`
	ItemDiscount      string `json:"item_discount"`
	AllocatedDiscount string `json:"allocated_discount"`
	Base              string `json:"base"`
	LineCost          string `json:"cost"`
}
type Calculation struct {
	CalculatorVersion int    `json:"calculator_version"`
	Gross             string `json:"gross"`
	ItemDiscount      string `json:"item_discount"`
	GeneralDiscount   string `json:"general_discount"`
	Base15            string `json:"base_15"`
	Base18            string `json:"base_18"`
	Exempt            string `json:"exempt"`
	Exonerated        string `json:"exonerated"`
	ISV15             string `json:"isv_15"`
	ISV18             string `json:"isv_18"`
	Total             string `json:"total"`
	Cost              string `json:"cost"`
	Margin            string `json:"margin"`
}
type CalculationInput struct {
	Items        []Item `json:"items"`
	DiscountType string `json:"general_discount_type"`
	Discount     string `json:"general_discount"`
	Exoneration  string `json:"exoneration_reference"`
}
type Result struct {
	Items       []CalculatedItem `json:"items"`
	Calculation Calculation      `json:"calculation"`
}

func Calculate(p CalculationInput) (Result, error) {
	out := Result{Items: []CalculatedItem{}}
	if len(p.Items) > 200 || !validText(p.Exoneration, 500, false) {
		return out, core.Invalid
	}
	gross, discount, net, cost := decimal.Zero, decimal.Zero, decimal.Zero, decimal.Zero
	nets := make([]decimal.Decimal, len(p.Items))
	for i, x := range p.Items {
		if !validText(x.Description, 4000, true) || !validText(x.Unit, 40, false) {
			return out, core.Invalid
		}
		q, e := number(x.Quantity, 4)
		if e != nil || !q.IsPositive() {
			return out, core.Invalid
		}
		price, e := number(x.Price, 6)
		if e != nil {
			return out, e
		}
		c, e := number(x.Cost, 6)
		if e != nil {
			return out, e
		}
		d, e := unitDiscount(price, x.DiscountType, x.Discount)
		if e != nil {
			return out, e
		}
		switch x.Tax {
		case "isv15", "isv18", "exempt":
		case "exonerated":
			if strings.TrimSpace(p.Exoneration) == "" {
				return out, core.Invalid
			}
		default:
			return out, core.Invalid
		}
		g := q.Mul(price).Round(2)
		n := q.Mul(price.Sub(d)).Round(2)
		lc := q.Mul(c).Round(2)
		if g.GreaterThan(moneyLimit) || lc.GreaterThan(moneyLimit) {
			return out, core.Invalid
		}
		nets[i] = n
		gross = gross.Add(g)
		discount = discount.Add(g.Sub(n))
		net = net.Add(n)
		cost = cost.Add(lc)
		x.Quantity = q.String()
		x.Price = price.String()
		x.Cost = c.String()
		dv, _ := decimal.NewFromString(x.Discount)
		x.Discount = dv.String()
		out.Items = append(out.Items, CalculatedItem{x, d.String(), g.StringFixed(2), g.Sub(n).StringFixed(2), "0.00", n.StringFixed(2), lc.StringFixed(2)})
	}
	scale := int32(2)
	if p.DiscountType == "percent" {
		scale = 4
	} else if p.DiscountType != "amount" {
		return out, core.Invalid
	}
	gd, e := number(p.Discount, scale)
	if e != nil {
		return out, e
	}
	if p.DiscountType == "percent" {
		if gd.GreaterThan(decimal.NewFromInt(100)) {
			return out, core.Invalid
		}
		gd = net.Mul(gd).Shift(-2).Round(2)
	}
	if gd.GreaterThan(net) {
		return out, core.Invalid
	}
	allocation := make([]decimal.Decimal, len(nets))
	remainders := make([]decimal.Decimal, len(nets))
	sum := decimal.Zero
	if gd.IsPositive() {
		for i, n := range nets {
			a, r := gd.Shift(2).Mul(n.Shift(2)).QuoRem(net.Shift(2), 0)
			allocation[i] = a
			remainders[i] = r
			sum = sum.Add(a)
		}
		remaining := gd.Shift(2).Sub(sum).IntPart()
		order := make([]int, len(nets))
		for i := range order {
			order[i] = i
		}
		sort.SliceStable(order, func(i, j int) bool { return remainders[order[i]].GreaterThan(remainders[order[j]]) })
		for i := int64(0); i < remaining; i++ {
			allocation[order[i]] = allocation[order[i]].Add(decimal.NewFromInt(1))
		}
	}
	b15, b18, ex, exo := decimal.Zero, decimal.Zero, decimal.Zero, decimal.Zero
	for i, n := range nets {
		a := allocation[i].Shift(-2)
		b := n.Sub(a)
		out.Items[i].AllocatedDiscount = a.StringFixed(2)
		out.Items[i].Base = b.StringFixed(2)
		switch p.Items[i].Tax {
		case "isv15":
			b15 = b15.Add(b)
		case "isv18":
			b18 = b18.Add(b)
		case "exempt":
			ex = ex.Add(b)
		case "exonerated":
			exo = exo.Add(b)
		}
	}
	t15 := b15.Mul(decimal.RequireFromString("0.15")).Round(2)
	t18 := b18.Mul(decimal.RequireFromString("0.18")).Round(2)
	total := net.Sub(gd).Add(t15).Add(t18)
	margin := net.Sub(gd).Sub(cost)
	for _, d := range []decimal.Decimal{gross, discount, net, cost, gd, total, b15, b18, ex, exo, t15, t18, margin.Abs()} {
		if d.GreaterThan(moneyLimit) {
			return out, core.Invalid
		}
	}
	out.Calculation = Calculation{1, gross.StringFixed(2), discount.StringFixed(2), gd.StringFixed(2), b15.StringFixed(2), b18.StringFixed(2), ex.StringFixed(2), exo.StringFixed(2), t15.StringFixed(2), t18.StringFixed(2), total.StringFixed(2), cost.StringFixed(2), margin.StringFixed(2)}
	return out, nil
}
