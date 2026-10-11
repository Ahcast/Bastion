package purchaseorders

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"reflect"
	"testing"
)

func TestCloneOrderPreservesCommercialSnapshotAndResetsOperationalFields(t *testing.T) {
	for _, currency := range []string{"HNL", "USD"} {
		t.Run(currency, func(t *testing.T) {
			source := PurchaseOrder{ID: "1", OrderNumber: "SOURCE", Status: "delivered", Customer: CustomerInput{Name: "Cliente"}, CustomerVersion: 7, Currency: currency, DiscountType: "percent", Discount: "5", Exoneration: "CONSTANCIA", Version: 9, CreatedBy: "2", TeamID: "3"}
			for i, tax := range []string{"isv15", "isv18", "exempt", "exonerated"} {
				source.Items = append(source.Items, PurchaseOrderItem{CalculatedItem: CalculatedItem{Item: Item{ID: core.WrapID(int64(i + 10)), Description: "Equipo", Unit: "unidad", Quantity: "2", UnitSalePrice: "100", UnitCost: "30.005", DiscountType: "amount", Discount: "10", TaxType: tax, SupplierID: "8", SupplierName: "Proveedor histórico", Tracking: "TRK", Comments: "Comprado"}}, SupplierVersion: 4, Status: "delivered"})
			}
			before := source
			o, e := cloneOrder(source, core.Actor{UserID: "5"}, CloneInput{OrderNumber: "NEW", OrderDate: "2026-10-11", AdditionalExpenses: "9.99"})
			if e != nil {
				t.Fatal(e)
			}
			if o.ID != "" || o.Status != "received" || o.SourceQuoteID != nil || o.Version != 1 || o.CreatedBy != "5" || o.CustomerVersion != 7 || o.Currency != currency || o.TeamID != "3" {
				t.Fatal(o)
			}
			if o.Calculation.Gross != "800.00" || o.Calculation.Base15 != "171.00" || o.Calculation.Base18 != "171.00" || o.Calculation.Total != "740.43" || o.Calculation.InvestmentTotal != "250.03" {
				t.Fatal(o.Calculation)
			}
			for _, x := range o.Items {
				if x.ID != "" || x.Status != "pending" || x.Tracking != "" || x.Comments != "" || x.SupplierName != "Proveedor histórico" || x.SupplierVersion != 4 {
					t.Fatal(x)
				}
			}
			if !reflect.DeepEqual(source, before) {
				t.Fatal("source mutated")
			}
		})
	}
}
