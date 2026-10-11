package integration

import (
	"fmt"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
)

func setPurchaseOrderViewScope(t *testing.T, s *core.Service, scope string) {
	t.Helper()
	if _, e := s.DB.Exec(`UPDATE role_permissions rp JOIN roles r ON r.id=rp.role_id SET rp.scope=? WHERE r.name='purchase-orders-all' AND rp.permission='ordenes.view'`, scope); e != nil {
		t.Fatal(e)
	}
}

func listOrderInput(number, customerName, description, itemComment, headerComment, sale string) purchaseorders.Input {
	p := purchaseOrderInput("", "")
	p.OrderNumber = number
	p.Customer = purchaseorders.CustomerInput{Name: customerName}
	p.Comments = headerComment
	p.RequiredDeliveryDate = nil
	p.Items[0].Description = description
	p.Items[0].Comments = itemComment
	p.Items[0].Quantity = "1"
	p.Items[0].UnitSalePrice = sale
	p.Items[0].UnitCost = "10"
	p.Items[0].Discount = "0"
	p.Items[0].TaxType = "exempt"
	p.Discount = "0"
	p.AdditionalExpenses = "0"
	return p
}

func TestPurchaseOrderListScopesFiltersAndPagination(t *testing.T) {
	s, cs, _, a := purchaseOrdersFixture(t)
	grantPurchaseOrders(t, s.Core, a, "all")
	grantCustomer(t, s.Core, a, "all")

	customerID := makeCustomer(t, cs, a, "po-list-customer", customerInput("Cliente Registrado"))
	own := listOrderInput("OC-LIST-OWN", "ignorado", "Cable propio", "comentario propio", "cabecera propia", "100")
	own.Customer.ID = customerID
	own.Customer.Name = "snapshot falso"
	created, e := s.Save(ctx, a, "", "po-list-own", own)
	ownID := customerResult(t, created, e)["id"]

	other := listOrderInput("OC-LIST-OTHER", "Cliente Beta", "Taladro buscable", "detalle ajeno", "cabecera beta", "200")
	created, e = s.Save(ctx, a, "", "po-list-other", other)
	otherID := customerResult(t, created, e)["id"]

	team := listOrderInput("OC-LIST-TEAM", "Cliente Gamma", "Bomba", "Comentario equipo especial", "cabecera gamma", "300")
	created, e = s.Save(ctx, a, "", "po-list-team", team)
	teamIDOrder := customerResult(t, created, e)["id"]

	res, e := s.Core.DB.Exec(`INSERT INTO users(email,name,password_hash,active,version,created_at,updated_at) SELECT 'po-other@example.test','Usuario ajeno',password_hash,TRUE,1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=?`, a.UserID)
	if e != nil {
		t.Fatal(e)
	}
	otherUserN, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	otherUserID := fmt.Sprint(otherUserN)

	res, e = s.Core.DB.Exec(`INSERT INTO teams(name) VALUES('Equipo OC listado')`)
	if e != nil {
		t.Fatal(e)
	}
	teamN, e := res.LastInsertId()
	if e != nil {
		t.Fatal(e)
	}
	teamID := fmt.Sprint(teamN)
	if _, e = s.Core.DB.Exec(`INSERT INTO team_members(team_id,user_id) VALUES(?,?)`, teamID, a.UserID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET created_by=? WHERE id=?`, otherUserID, otherID); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Core.DB.Exec(`UPDATE customer_purchase_orders SET created_by=?,team_id=? WHERE id=?`, otherUserID, teamID, teamIDOrder); e != nil {
		t.Fatal(e)
	}

	setPurchaseOrderViewScope(t, s.Core, "self")
	page, e := s.List(ctx, a, purchaseorders.Filter{Page: 1, PerPage: 10})
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != ownID || page.Data[0].Customer.ID != customerID || page.Data[0].Customer.Name != "Cliente Registrado" {
		t.Fatal("self scope", page, e)
	}

	setPurchaseOrderViewScope(t, s.Core, "team")
	page, e = s.List(ctx, a, purchaseorders.Filter{Page: 1, PerPage: 10})
	if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != teamIDOrder {
		t.Fatal("team scope", page, e)
	}

	setPurchaseOrderViewScope(t, s.Core, "all")
	page, e = s.List(ctx, a, purchaseorders.Filter{Page: 1, PerPage: 2})
	if e != nil || page.Total != 3 || len(page.Data) != 2 || page.Page != 1 || page.PerPage != 2 {
		t.Fatal("all first page", page, e)
	}
	page2, e := s.List(ctx, a, purchaseorders.Filter{Page: 2, PerPage: 2})
	if e != nil || page2.Total != 3 || len(page2.Data) != 1 {
		t.Fatal("all second page", page2, e)
	}

	checks := []struct {
		name   string
		filter purchaseorders.Filter
		id     string
	}{
		{"order number", purchaseorders.Filter{Search: "OC-LIST-OTHER", Page: 1, PerPage: 10}, otherID},
		{"customer", purchaseorders.Filter{Search: "Cliente Beta", Page: 1, PerPage: 10}, otherID},
		{"item description", purchaseorders.Filter{Search: "Taladro buscable", Page: 1, PerPage: 10}, otherID},
		{"item comment", purchaseorders.Filter{Search: "Comentario equipo especial", Page: 1, PerPage: 10}, teamIDOrder},
		{"header comment", purchaseorders.Filter{Search: "cabecera propia", Page: 1, PerPage: 10}, ownID},
		{"customer id", purchaseorders.Filter{CustomerID: customerID, Page: 1, PerPage: 10}, ownID},
		{"total range", purchaseorders.Filter{TotalMin: "150", TotalMax: "250.00", Page: 1, PerPage: 10}, otherID},
		{"status", purchaseorders.Filter{Status: "received", TotalMin: "250", Page: 1, PerPage: 10}, teamIDOrder},
	}
	for _, check := range checks {
		page, e = s.List(ctx, a, check.filter)
		if e != nil || page.Total != 1 || len(page.Data) != 1 || page.Data[0].ID != check.id {
			t.Fatalf("%s: page=%+v error=%v", check.name, page, e)
		}
	}

	invalid := []purchaseorders.Filter{
		{Page: 0, PerPage: 10},
		{Page: 1, PerPage: 0},
		{Page: 1, PerPage: 101},
		{CustomerID: "bad", Page: 1, PerPage: 10},
		{Status: "confirmed", Page: 1, PerPage: 10},
		{TotalMin: ".5", Page: 1, PerPage: 10},
		{TotalMax: "1e2", Page: 1, PerPage: 10},
		{TotalMin: "10", TotalMax: "9.99", Page: 1, PerPage: 10},
	}
	for i, f := range invalid {
		if _, e := s.List(ctx, a, f); e != core.Invalid {
			t.Fatalf("invalid filter %d: %v", i, e)
		}
	}
}
