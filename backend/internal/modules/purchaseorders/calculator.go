package purchaseorders

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
)

var investmentLimit = decimal.RequireFromString("9999999999999999.99")

type Item struct {
	Description   string `json:"description"`
	Unit          string `json:"unit"`
	Quantity      string `json:"quantity"`
	UnitSalePrice string `json:"unit_sale_price"`
	UnitCost      string `json:"unit_cost"`
	DiscountType  string `json:"discount_type"`
	Discount      string `json:"discount"`
	TaxType       string `json:"tax_type"`
	SupplierID    string `json:"supplier_id"`
	SupplierName  string `json:"supplier_name_snapshot"`
	Tracking      string `json:"tracking"`
	Comments      string `json:"comments"`
}

type CalculatedItem struct {
	Item
	UnitDiscount      string `json:"unit_discount"`
	Gross             string `json:"gross"`
	ItemDiscount      string `json:"item_discount"`
	AllocatedDiscount string `json:"allocated_discount"`
	Base              string `json:"base"`
	LineInvestment    string `json:"line_investment"`
}

type CalculationInput struct {
	Items              []Item `json:"items"`
	DiscountType       string `json:"general_discount_type"`
	Discount           string `json:"general_discount"`
	Exoneration        string `json:"exoneration_reference"`
	AdditionalExpenses string `json:"additional_expenses"`
}

type Calculation struct {
	Gross              string `json:"gross"`
	ItemDiscount       string `json:"item_discount"`
	GeneralDiscount    string `json:"general_discount"`
	Base15             string `json:"base_15"`
	Base18             string `json:"base_18"`
	Exempt             string `json:"exempt"`
	Exonerated         string `json:"exonerated"`
	ISV15              string `json:"isv_15"`
	ISV18              string `json:"isv_18"`
	Total              string `json:"total"`
	ProductsInvestment string `json:"products_investment"`
	AdditionalExpenses string `json:"additional_expenses"`
	InvestmentTotal    string `json:"investment_total"`
	CalculatorVersion  int    `json:"calculator_version"`
}

type CalculationResult struct {
	Items       []CalculatedItem `json:"items"`
	Calculation Calculation      `json:"calculation"`
}

func Calculate(p CalculationInput) (CalculationResult, error) {
	out := CalculationResult{Items: []CalculatedItem{}}
	if len(p.Items) == 0 || len(p.Items) > 200 {
		return out, core.Invalid
	}

	items := make([]finance.Item, 0, len(p.Items))
	for _, x := range p.Items {
		items = append(items, finance.Item{
			Description:  x.Description,
			Unit:         x.Unit,
			Quantity:     x.Quantity,
			Price:        x.UnitSalePrice,
			Cost:         x.UnitCost,
			DiscountType: x.DiscountType,
			Discount:     x.Discount,
			Tax:          x.TaxType,
		})
	}

	sales, e := finance.Calculate(finance.CalculationInput{
		Items:        items,
		DiscountType: p.DiscountType,
		Discount:     p.Discount,
		Exoneration:  p.Exoneration,
	})
	if e != nil {
		return out, e
	}
	additional, e := finance.ParseDecimal(p.AdditionalExpenses, 2)
	if e != nil {
		return out, e
	}
	products := decimal.RequireFromString(sales.Calculation.Cost)
	investment := products.Add(additional)
	if investment.GreaterThan(investmentLimit) {
		return out, core.Invalid
	}

	for i, x := range sales.Items {
		original := p.Items[i]
		original.Description = x.Description
		original.Unit = x.Unit
		original.Quantity = x.Quantity
		original.UnitSalePrice = x.Price
		original.UnitCost = x.Cost
		original.DiscountType = x.DiscountType
		original.Discount = x.Discount
		original.TaxType = x.Tax
		out.Items = append(out.Items, CalculatedItem{
			Item:              original,
			UnitDiscount:      x.UnitDiscount,
			Gross:             x.Gross,
			ItemDiscount:      x.ItemDiscount,
			AllocatedDiscount: x.AllocatedDiscount,
			Base:              x.Base,
			LineInvestment:    x.LineCost,
		})
	}

	c := sales.Calculation
	out.Calculation = Calculation{
		Gross:              c.Gross,
		ItemDiscount:       c.ItemDiscount,
		GeneralDiscount:    c.GeneralDiscount,
		Base15:             c.Base15,
		Base18:             c.Base18,
		Exempt:             c.Exempt,
		Exonerated:         c.Exonerated,
		ISV15:              c.ISV15,
		ISV18:              c.ISV18,
		Total:              c.Total,
		ProductsInvestment: products.StringFixed(2),
		AdditionalExpenses: additional.StringFixed(2),
		InvestmentTotal:    investment.StringFixed(2),
		CalculatorVersion:  c.CalculatorVersion,
	}
	return out, nil
}
