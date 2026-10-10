package quotes

import (
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
)

// Quotes keeps its public DTO and Calculate surface for compatibility while
// the authoritative commercial calculation lives in internal/finance.
type Item = finance.Item
type CalculatedItem = finance.CalculatedItem
type Calculation = finance.Calculation
type CalculationInput = finance.CalculationInput
type Result = finance.Result

func number(v string, scale int32) (decimal.Decimal, error) {
	return finance.ParseDecimal(v, scale)
}

func validText(v string, n int, required bool) bool {
	return finance.ValidText(v, n, required)
}

func Calculate(p CalculationInput) (Result, error) {
	return finance.Calculate(p)
}
