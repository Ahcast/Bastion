package purchaseorders

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type CloneInput struct {
	Version              uint64  `json:"version,string"`
	OrderNumber          string  `json:"order_number"`
	OrderDate            string  `json:"order_date"`
	RequiredDeliveryDate *string `json:"required_delivery_date"`
	Destination          string  `json:"destination"`
	Comments             string  `json:"comments"`
	AdditionalExpenses   string  `json:"additional_expenses"`
}

func cloneOrder(source PurchaseOrder, actor core.Actor, meta CloneInput) (PurchaseOrder, error) {
	items := make([]Item, len(source.Items))
	for i, x := range source.Items {
		items[i] = x.Item
		items[i].ID = ""
		items[i].Tracking = ""
		items[i].Comments = ""
	}
	input := Input{OrderNumber: meta.OrderNumber, Customer: source.Customer, Destination: meta.Destination, OrderDate: meta.OrderDate, RequiredDeliveryDate: meta.RequiredDeliveryDate, Currency: source.Currency, Comments: meta.Comments, TeamID: source.TeamID, CalculationInput: CalculationInput{Items: items, DiscountType: source.DiscountType, Discount: source.Discount, Exoneration: source.Exoneration, AdditionalExpenses: meta.AdditionalExpenses}}
	calculated, e := Validate(&input)
	if e != nil {
		return PurchaseOrder{}, e
	}
	o := PurchaseOrder{OrderNumber: input.OrderNumber, Status: "received", Customer: source.Customer, CustomerVersion: source.CustomerVersion, Destination: input.Destination, OrderDate: input.OrderDate, RequiredDeliveryDate: input.RequiredDeliveryDate, Currency: source.Currency, Comments: input.Comments, DiscountType: input.DiscountType, Discount: input.Discount, Exoneration: input.Exoneration, Calculation: calculated.Calculation, Version: 1, CreatedBy: actor.UserID, TeamID: source.TeamID, Items: []PurchaseOrderItem{}}
	for i, x := range calculated.Items {
		o.Items = append(o.Items, PurchaseOrderItem{CalculatedItem: x, SupplierVersion: source.Items[i].SupplierVersion, Status: "pending"})
	}
	return o, nil
}

func (s *Service) Clone(ctx context.Context, a core.Actor, id, key string, p CloneInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if core.ID(id) != nil || p.Version == 0 {
		return nil, core.Invalid
	}
	var out any
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		source, e := s.access(ctx, tx, a, id, "ordenes.view", true)
		if e != nil {
			return e
		}
		if e = s.AuthorizeQuoteConversion(ctx, tx, a, source.TeamID, nil); e != nil {
			return e
		}
		if e = s.items(ctx, tx, &source); e != nil {
			return e
		}
		o, e := cloneOrder(source, a, p)
		if e != nil {
			return e
		}
		out, e = s.Core.Idempotent(ctx, tx, a, "purchase_orders.clone:"+id, key, p, func() (any, error) {
			if source.Version != p.Version {
				return nil, core.Conflict
			}
			result, e := s.persist(ctx, tx, a, &o)
			if e != nil {
				return nil, e
			}
			if _, e = tx.ExecContext(ctx, `INSERT INTO customer_purchase_order_clones(purchase_order_id,source_purchase_order_id,source_version,created_by,created_at) VALUES(?,?,?,?,UTC_TIMESTAMP(6))`, o.ID, source.ID, source.Version, a.UserID); e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "purchase_order.cloned", "customer_purchase_orders", o.ID, "success", map[string]string{"source_id": source.ID, "source_version": fmt.Sprint(source.Version)}); e != nil {
				return nil, e
			}
			if e = s.Core.EmitEvent(ctx, tx, a, "purchase_order.cloned", "customer_purchase_orders", o.ID, o.Version); e != nil {
				return nil, e
			}
			return result, nil
		})
		if e != nil {
			return e
		}
		raw, e := json.Marshal(out)
		if e != nil {
			return e
		}
		var result struct {
			ID string `json:"id"`
		}
		if e = json.Unmarshal(raw, &result); e != nil {
			return e
		}
		_, e = s.access(ctx, tx, a, result.ID, "ordenes.view", true)
		return e
	})
	return out, orderDBError(e)
}
