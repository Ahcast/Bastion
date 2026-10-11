package integration

import (
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
)

func purchaseOrdersFixture(t *testing.T) (*purchaseorders.Service, *customers.Service, *suppliers.Service, core.Actor) {
	t.Helper()
	cs, a, _ := customersFixture(t)
	ss := &suppliers.Service{Core: cs.Core}
	return &purchaseorders.Service{Core: cs.Core, Customers: cs, Suppliers: ss}, cs, ss, a
}

func grantPurchaseOrders(t *testing.T, s *core.Service, a core.Actor, scope string) {
	t.Helper()
	name := "purchase-orders-" + scope
	if _, e := s.DB.Exec(`INSERT IGNORE INTO roles(name) VALUES(?)`, name); e != nil {
		t.Fatal(e)
	}
	var id string
	if e := s.DB.QueryRow(`SELECT id FROM roles WHERE name=?`, name).Scan(&id); e != nil {
		t.Fatal(e)
	}
	for _, p := range purchaseorders.Permissions {
		if _, e := s.DB.Exec(`INSERT INTO role_permissions VALUES(?,?,?) ON DUPLICATE KEY UPDATE scope=VALUES(scope)`, id, p, scope); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.DB.Exec(`INSERT IGNORE INTO user_roles VALUES(?,?)`, a.UserID, id); e != nil {
		t.Fatal(e)
	}
}

func purchaseOrderInput(customerID, supplierID string) purchaseorders.Input {
	delivery := "2026-11-15"
	return purchaseorders.Input{
		OrderNumber: "OC-CLIENTE-001",
		Customer: purchaseorders.CustomerInput{
			ID:   customerID,
			Name: "Snapshot enviado por cliente",
		},
		Destination:          "Patuca III",
		OrderDate:            "2026-10-10",
		RequiredDeliveryDate: &delivery,
		Currency:             "HNL",
		Comments:             "Orden sintética",
		CalculationInput: purchaseorders.CalculationInput{
			Items: []purchaseorders.Item{{
				Description:   "Equipo",
				Unit:          "unidad",
				Quantity:      "2",
				UnitSalePrice: "100",
				UnitCost:      "30.005",
				DiscountType:  "amount",
				Discount:      "10",
				TaxType:       "isv15",
				SupplierID:    supplierID,
				SupplierName:  "Snapshot enviado por cliente",
			}},
			DiscountType:       "amount",
			Discount:           "0",
			AdditionalExpenses: "9.99",
		},
	}
}

func editPurchaseOrder(o purchaseorders.PurchaseOrder) purchaseorders.Input {
	items := make([]purchaseorders.Item, 0, len(o.Items))
	for _, x := range o.Items {
		items = append(items, x.Item)
	}
	return purchaseorders.Input{
		OrderNumber:          o.OrderNumber,
		Customer:             o.Customer,
		Destination:          o.Destination,
		OrderDate:            o.OrderDate,
		RequiredDeliveryDate: o.RequiredDeliveryDate,
		Currency:             o.Currency,
		Comments:             o.Comments,
		CalculationInput: purchaseorders.CalculationInput{
			Items:              items,
			DiscountType:       o.DiscountType,
			Discount:           o.Discount,
			Exoneration:        o.Exoneration,
			AdditionalExpenses: o.Calculation.AdditionalExpenses,
		},
		TeamID:  o.TeamID,
		Version: o.Version,
	}
}

func TestPurchaseOrdersTransactionalSaveGetSnapshotsIdempotencyAndRollback(t *testing.T) {
	s, cs, ss, a := purchaseOrdersFixture(t)
	manual := purchaseOrderInput("", "")
	if _, e := s.Save(ctx, a, "", "po-ungranted", manual); e != core.Forbidden {
		t.Fatal("technical admin gained order permission", e)
	}

	grantPurchaseOrders(t, s.Core, a, "all")
	grantCustomer(t, s.Core, a, "all")
	grantSupplier(t, s.Core, a, "all")
	customerID := makeCustomer(t, cs, a, "po-customer", customerInput("Cliente OC"))
	supplierID := makeSupplier(t, ss, a, "po-supplier", supplierInput("Proveedor OC"))

	p := purchaseOrderInput(customerID, supplierID)
	created, e := s.Save(ctx, a, "", "po-create", p)
	result := customerResult(t, created, e)
	id := result["id"]
	if result["version"] != "1" {
		t.Fatal(result)
	}
	replayed, e := s.Save(ctx, a, "", "po-create", p)
	if e != nil || customerResult(t, replayed, e)["id"] != id {
		t.Fatal("create replay", replayed, e)
	}

	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	if o.Status != "received" || o.SourceQuoteID != nil || o.Customer.ID != customerID || o.Customer.Name != "Cliente OC" || o.CustomerVersion != 1 || o.Version != 1 {
		t.Fatal(o)
	}
	if o.Calculation.GeneralDiscount != "0.00" || o.Calculation.Total != "207.00" || o.Calculation.ProductsInvestment != "60.01" || o.Calculation.InvestmentTotal != "70.00" {
		t.Fatal(o.Calculation)
	}
	if len(o.Items) != 1 || o.Items[0].Status != "pending" || o.Items[0].SupplierID != supplierID || o.Items[0].SupplierName != "Proveedor OC" || o.Items[0].SupplierVersion != 1 || o.Items[0].LineInvestment != "60.01" {
		t.Fatal(o.Items)
	}

	edit := editPurchaseOrder(o)
	edit.Comments = "Comentario actualizado"
	edit.Discount = "5"
	updated, e := s.Save(ctx, a, id, "po-update", edit)
	if customerResult(t, updated, e)["version"] != "2" {
		t.Fatal(updated, e)
	}
	if _, e := s.Save(ctx, a, id, "po-stale", edit); e != core.Conflict {
		t.Fatal("stale update", e)
	}
	if _, e := s.Save(ctx, a, id, "po-update", edit); e != nil {
		t.Fatal("update replay", e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || o.Version != 2 || o.Comments != "Comentario actualizado" || o.Calculation.GeneralDiscount != "5.00" || o.Calculation.Total != "201.25" || o.Customer.Name != "Cliente OC" || o.Items[0].SupplierName != "Proveedor OC" {
		t.Fatal(o, e)
	}

	if _, e := s.Core.DB.Exec(`CREATE TRIGGER test_po_audit BEFORE INSERT ON audit_events FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='synthetic outage'`); e != nil {
		t.Fatal(e)
	}
	failed := purchaseOrderInput("", "")
	failed.OrderNumber = "OC-ROLLBACK"
	if _, e := s.Save(ctx, a, "", "po-rollback", failed); e == nil {
		t.Fatal("audit failure swallowed")
	}
	var orders, items int
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_orders`).Scan(&orders); e != nil || orders != 1 {
		t.Fatal("orders rollback", orders, e)
	}
	if e := s.Core.DB.QueryRow(`SELECT COUNT(*) FROM customer_purchase_order_items`).Scan(&items); e != nil || items != 1 {
		t.Fatal("items rollback", items, e)
	}
	if _, e := s.Core.DB.Exec(`DROP TRIGGER test_po_audit`); e != nil {
		t.Fatal(e)
	}
	created, e = s.Save(ctx, a, "", "po-rollback", failed)
	if e != nil || customerResult(t, created, e)["version"] != "1" {
		t.Fatal("rolled-back idempotency record survived", created, e)
	}
}

func TestPurchaseOrdersReorderPreservesSupplierSnapshots(t *testing.T) {
	s, _, ss, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	grantSupplier(t, s.Core, a, "all")

	supplierA := makeSupplier(t, ss, a, "po-reorder-a", supplierInput("Proveedor A"))
	supplierB := makeSupplier(t, ss, a, "po-reorder-b", supplierInput("Proveedor B"))
	p := purchaseOrderInput("", supplierA)
	p.OrderNumber = "OC-REORDER"
	second := p.Items[0]
	second.Description = "Equipo B"
	second.SupplierID = supplierB
	p.Items = append(p.Items, second)

	created, e := s.Save(ctx, a, "", "po-reorder-create", p)
	id := customerResult(t, created, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil || len(o.Items) != 2 {
		t.Fatal(o, e)
	}
	if o.Items[0].SupplierName != "Proveedor A" || o.Items[1].SupplierName != "Proveedor B" {
		t.Fatal(o.Items)
	}

	if _, e := s.Core.DB.Exec(`UPDATE suppliers SET name=CONCAT(name,' actualizado'),version=version+1,updated_at=UTC_TIMESTAMP(6) WHERE id IN (?,?)`, supplierA, supplierB); e != nil {
		t.Fatal(e)
	}
	edit := editPurchaseOrder(o)
	edit.Items[0], edit.Items[1] = edit.Items[1], edit.Items[0]
	if _, e := s.Save(ctx, a, id, "po-reorder-update", edit); e != nil {
		t.Fatal(e)
	}
	o, e = s.Get(ctx, a, id)
	if e != nil || len(o.Items) != 2 {
		t.Fatal(o, e)
	}
	if o.Items[0].SupplierID != supplierB || o.Items[0].SupplierName != "Proveedor B" || o.Items[0].SupplierVersion != 1 {
		t.Fatal("supplier B snapshot changed on reorder", o.Items[0])
	}
	if o.Items[1].SupplierID != supplierA || o.Items[1].SupplierName != "Proveedor A" || o.Items[1].SupplierVersion != 1 {
		t.Fatal("supplier A snapshot changed on reorder", o.Items[1])
	}
}
