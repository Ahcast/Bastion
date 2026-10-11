package purchaseorders

import (
	"context"
	"database/sql"
	"errors"
	"fmt"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"github.com/go-sql-driver/mysql"
	"github.com/shopspring/decimal"
)

type CustomerSource interface {
	SnapshotInTransaction(context.Context, *sql.Tx, core.Actor, string) (customers.Snapshot, error)
}

type SupplierSource interface {
	SnapshotInTransaction(context.Context, *sql.Tx, core.Actor, string) (suppliers.Snapshot, error)
}

type Service struct {
	Core      *core.Service
	Customers CustomerSource
	Suppliers SupplierSource
}

func nullable(v string) any {
	if v == "" {
		return nil
	}
	return v
}

func nullableDate(v *string) any {
	if v == nil {
		return nil
	}
	return *v
}

func orderDBError(e error) error {
	var m *mysql.MySQLError
	if errors.As(e, &m) && (m.Number == 1062 || m.Number == 1452) {
		return core.Conflict
	}
	return e
}

func (s *Service) gate(ctx context.Context, q core.Querier, a core.Actor, permission string) error {
	var team string
	e := q.QueryRowContext(ctx, `SELECT team_id FROM team_members WHERE user_id=? ORDER BY team_id LIMIT 1`, a.UserID).Scan(&team)
	if e != nil && e != sql.ErrNoRows {
		return e
	}
	return s.Core.Authorize(ctx, q, a, permission, a.UserID, team, false)
}

const orderColumns = `o.id,o.order_number,o.status,COALESCE(CAST(o.source_quote_id AS CHAR),''),COALESCE(CAST(o.customer_id AS CHAR),''),o.customer_version,o.customer_name_snapshot,o.customer_rtn_snapshot,o.customer_contact_snapshot,o.customer_phone_snapshot,o.customer_email_snapshot,o.customer_address_snapshot,o.destination,CAST(o.order_date AS CHAR),COALESCE(CAST(o.required_delivery_date AS CHAR),''),o.currency,o.comments,o.exoneration_reference,o.general_discount_type,CAST(o.general_discount AS CHAR),CAST(o.gross AS CHAR),CAST(o.item_discount AS CHAR),CAST(o.allocated_discount AS CHAR),CAST(o.base_15 AS CHAR),CAST(o.base_18 AS CHAR),CAST(o.exempt AS CHAR),CAST(o.exonerated AS CHAR),CAST(o.isv_15 AS CHAR),CAST(o.isv_18 AS CHAR),CAST(o.total AS CHAR),CAST(o.products_investment AS CHAR),CAST(o.additional_expenses AS CHAR),CAST(o.investment_total AS CHAR),o.calculator_version,o.version,CAST(o.created_by AS CHAR),COALESCE(CAST(o.team_id AS CHAR),'')`

func scanOrder(row interface{ Scan(...any) error }) (PurchaseOrder, error) {
	var o PurchaseOrder
	var source, delivery, allocated string
	o.Items = []PurchaseOrderItem{}
	c := &o.Calculation
	e := row.Scan(
		&o.ID, &o.OrderNumber, &o.Status, &source, &o.Customer.ID, &o.CustomerVersion,
		&o.Customer.Name, &o.Customer.RTN, &o.Customer.Contact, &o.Customer.Phone, &o.Customer.Email, &o.Customer.Address,
		&o.Destination, &o.OrderDate, &delivery, &o.Currency, &o.Comments, &o.Exoneration, &o.DiscountType, &o.Discount,
		&c.Gross, &c.ItemDiscount, &allocated, &c.Base15, &c.Base18, &c.Exempt, &c.Exonerated, &c.ISV15, &c.ISV18, &c.Total,
		&c.ProductsInvestment, &c.AdditionalExpenses, &c.InvestmentTotal, &c.CalculatorVersion,
		&o.Version, &o.CreatedBy, &o.TeamID,
	)
	if e != nil {
		return o, e
	}
	if source != "" {
		o.SourceQuoteID = &source
	}
	if delivery != "" {
		o.RequiredDeliveryDate = &delivery
	}
	o.Discount = decimal.RequireFromString(o.Discount).String()
	return o, nil
}

func (s *Service) access(ctx context.Context, q core.Querier, a core.Actor, id, permission string, lock bool) (PurchaseOrder, error) {
	if core.ID(id) != nil {
		return PurchaseOrder{}, core.Invalid
	}
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	o, e := scanOrder(q.QueryRowContext(ctx, `SELECT `+orderColumns+` FROM customer_purchase_orders o WHERE o.id=?`+suffix, id))
	if e == sql.ErrNoRows {
		return o, core.Missing
	}
	if e != nil {
		return o, e
	}
	if e = s.Core.Authorize(ctx, q, a, permission, o.CreatedBy, o.TeamID, false); e != nil {
		if e == core.Forbidden {
			return PurchaseOrder{}, core.Missing
		}
		return PurchaseOrder{}, e
	}
	return o, nil
}

func (s *Service) items(ctx context.Context, q core.Querier, o *PurchaseOrder) error {
	rows, e := q.QueryContext(ctx, `SELECT description,unit,CAST(quantity AS CHAR),CAST(unit_sale_price AS CHAR),CAST(unit_cost AS CHAR),discount_type,CAST(discount AS CHAR),tax_type,COALESCE(CAST(supplier_id AS CHAR),''),supplier_version,supplier_name_snapshot,status,tracking,comments,CAST(unit_discount AS CHAR),CAST(gross AS CHAR),CAST(item_discount AS CHAR),CAST(allocated_discount AS CHAR),CAST(base AS CHAR),CAST(line_investment AS CHAR) FROM customer_purchase_order_items WHERE purchase_order_id=? ORDER BY position`, o.ID)
	if e != nil {
		return e
	}
	defer rows.Close()
	for rows.Next() {
		var x PurchaseOrderItem
		if e = rows.Scan(
			&x.Description, &x.Unit, &x.Quantity, &x.UnitSalePrice, &x.UnitCost, &x.DiscountType, &x.Discount, &x.TaxType,
			&x.SupplierID, &x.SupplierVersion, &x.SupplierName, &x.Status, &x.Tracking, &x.Comments,
			&x.UnitDiscount, &x.Gross, &x.ItemDiscount, &x.AllocatedDiscount, &x.Base, &x.LineInvestment,
		); e != nil {
			return e
		}
		x.Quantity = decimal.RequireFromString(x.Quantity).String()
		x.UnitSalePrice = decimal.RequireFromString(x.UnitSalePrice).String()
		x.UnitCost = decimal.RequireFromString(x.UnitCost).String()
		x.Discount = decimal.RequireFromString(x.Discount).String()
		x.UnitDiscount = decimal.RequireFromString(x.UnitDiscount).String()
		o.Items = append(o.Items, x)
	}
	return rows.Err()
}

func (s *Service) Get(ctx context.Context, a core.Actor, id string) (PurchaseOrder, error) {
	var o PurchaseOrder
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "ordenes.view"); e != nil {
			return e
		}
		var e error
		o, e = s.access(ctx, tx, a, id, "ordenes.view", false)
		if e != nil {
			return e
		}
		return s.items(ctx, tx, &o)
	})
	return o, e
}

func customerSnapshot(c customers.Snapshot) CustomerInput {
	return CustomerInput{ID: c.ID, Name: c.Name, RTN: c.RTN, Contact: c.Contact, Phone: c.Phone, Email: c.Email, Address: c.Address}
}

func (s *Service) resolveCustomer(ctx context.Context, tx *sql.Tx, a core.Actor, p Input, old PurchaseOrder) (CustomerInput, uint64, error) {
	if p.Customer.ID == "" {
		return p.Customer, 0, nil
	}
	if old.ID != "" && p.Customer.ID == old.Customer.ID {
		return old.Customer, old.CustomerVersion, nil
	}
	if s.Customers == nil {
		return CustomerInput{}, 0, core.Forbidden
	}
	c, e := s.Customers.SnapshotInTransaction(ctx, tx, a, p.Customer.ID)
	if e != nil {
		return CustomerInput{}, 0, e
	}
	return customerSnapshot(c), c.Version, nil
}

func (s *Service) resolveItems(ctx context.Context, tx *sql.Tx, a core.Actor, calculated CalculationResult, old PurchaseOrder) ([]PurchaseOrderItem, error) {
	out := make([]PurchaseOrderItem, 0, len(calculated.Items))
	oldBySupplier := map[string][]PurchaseOrderItem{}
	for _, oldItem := range old.Items {
		if oldItem.SupplierID != "" {
			oldBySupplier[oldItem.SupplierID] = append(oldBySupplier[oldItem.SupplierID], oldItem)
		}
	}
	usedBySupplier := map[string]int{}

	for _, x := range calculated.Items {
		item := PurchaseOrderItem{CalculatedItem: x, Status: "pending"}
		if x.SupplierID == "" {
			out = append(out, item)
			continue
		}
		if snapshots := oldBySupplier[x.SupplierID]; usedBySupplier[x.SupplierID] < len(snapshots) {
			oldItem := snapshots[usedBySupplier[x.SupplierID]]
			usedBySupplier[x.SupplierID]++
			item.SupplierName = oldItem.SupplierName
			item.SupplierVersion = oldItem.SupplierVersion
			out = append(out, item)
			continue
		}
		if s.Suppliers == nil {
			return nil, core.Forbidden
		}
		v, e := s.Suppliers.SnapshotInTransaction(ctx, tx, a, x.SupplierID)
		if e != nil {
			return nil, e
		}
		if v.ID == nil {
			return nil, core.Missing
		}
		item.SupplierID = *v.ID
		item.SupplierName = v.Name
		item.SupplierVersion = v.Version
		out = append(out, item)
	}
	return out, nil
}

func (s *Service) authorizeCreateTeam(ctx context.Context, tx *sql.Tx, a core.Actor, permission, teamID string) error {
	if e := s.Core.Authorize(ctx, tx, a, permission, a.UserID, teamID, false); e != nil {
		return e
	}
	if teamID == "" {
		return nil
	}
	var member int
	if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?`, teamID, a.UserID).Scan(&member); e != nil {
		return e
	}
	if member == 0 {
		return s.Core.Authorize(ctx, tx, a, permission, "", "", false)
	}
	return nil
}

func (s *Service) Save(ctx context.Context, a core.Actor, id, key string, p Input) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	calculated, e := Validate(&p)
	if e != nil {
		return nil, e
	}
	if id == "" && p.Version != 0 {
		return nil, core.Invalid
	}
	if id != "" && (core.ID(id) != nil || p.Version == 0) {
		return nil, core.Invalid
	}

	var out any
	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		permission := "ordenes.create"
		if id != "" {
			permission = "ordenes.update"
		}
		if e := s.gate(ctx, tx, a, permission); e != nil {
			return e
		}

		var old PurchaseOrder
		if id == "" {
			if e := s.authorizeCreateTeam(ctx, tx, a, permission, p.TeamID); e != nil {
				return e
			}
		} else {
			var e error
			old, e = s.access(ctx, tx, a, id, permission, true)
			if e != nil {
				return e
			}
			if e = s.items(ctx, tx, &old); e != nil {
				return e
			}
			if p.TeamID != old.TeamID {
				if e := s.Core.Authorize(ctx, tx, a, permission, "", "", true); e != nil {
					return e
				}
			}
		}

		out, e = s.Core.Idempotent(ctx, tx, a, "purchase_orders.save:"+id, key, p, func() (any, error) {
			if id != "" && (old.Version != p.Version || !editableOrder(old)) {
				return nil, core.Conflict
			}
			customer, customerVersion, e := s.resolveCustomer(ctx, tx, a, p, old)
			if e != nil {
				return nil, e
			}
			resolvedItems, e := s.resolveItems(ctx, tx, a, calculated, old)
			if e != nil {
				return nil, e
			}
			o := PurchaseOrder{
				ID:                   id,
				OrderNumber:          p.OrderNumber,
				Status:               "received",
				Customer:             customer,
				CustomerVersion:      customerVersion,
				SourceQuoteID:        old.SourceQuoteID,
				Destination:          p.Destination,
				OrderDate:            p.OrderDate,
				RequiredDeliveryDate: p.RequiredDeliveryDate,
				Currency:             p.Currency,
				Comments:             p.Comments,
				DiscountType:         p.DiscountType,
				Discount:             p.Discount,
				Exoneration:          p.Exoneration,
				Calculation:          calculated.Calculation,
				Version:              old.Version + 1,
				CreatedBy:            old.CreatedBy,
				TeamID:               p.TeamID,
				Items:                resolvedItems,
			}
			if id == "" {
				o.Version = 1
				o.CreatedBy = a.UserID
			}
			return s.persist(ctx, tx, a, &o)
		})
		return e
	})
	return out, orderDBError(e)
}

func (s *Service) persist(ctx context.Context, tx *sql.Tx, a core.Actor, o *PurchaseOrder) (any, error) {
	c := o.Calculation
	topic := "purchase_order.created"
	if o.ID == "" {
		res, e := tx.ExecContext(ctx, `INSERT INTO customer_purchase_orders(order_number,status,source_quote_id,customer_id,customer_version,customer_name_snapshot,customer_rtn_snapshot,customer_contact_snapshot,customer_phone_snapshot,customer_email_snapshot,customer_address_snapshot,destination,order_date,required_delivery_date,currency,comments,exoneration_reference,general_discount_type,general_discount,gross,item_discount,allocated_discount,base_15,base_18,exempt,exonerated,isv_15,isv_18,total,products_investment,additional_expenses,investment_total,calculator_version,version,created_by,updated_by,team_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`,
			o.OrderNumber, o.Status, nil, nullable(o.Customer.ID), o.CustomerVersion, o.Customer.Name, o.Customer.RTN, o.Customer.Contact, o.Customer.Phone, o.Customer.Email, o.Customer.Address,
			o.Destination, o.OrderDate, nullableDate(o.RequiredDeliveryDate), o.Currency, o.Comments, o.Exoneration, o.DiscountType, o.Discount,
			c.Gross, c.ItemDiscount, c.GeneralDiscount, c.Base15, c.Base18, c.Exempt, c.Exonerated, c.ISV15, c.ISV18, c.Total,
			c.ProductsInvestment, c.AdditionalExpenses, c.InvestmentTotal, c.CalculatorVersion, o.Version, o.CreatedBy, a.UserID, nullable(o.TeamID),
		)
		if e != nil {
			return nil, orderDBError(e)
		}
		n, e := res.LastInsertId()
		if e != nil {
			return nil, e
		}
		o.ID = core.WrapID(n)
	} else {
		topic = "purchase_order.updated"
		if e := core.Changed(tx.ExecContext(ctx, `UPDATE customer_purchase_orders SET order_number=?,customer_id=?,customer_version=?,customer_name_snapshot=?,customer_rtn_snapshot=?,customer_contact_snapshot=?,customer_phone_snapshot=?,customer_email_snapshot=?,customer_address_snapshot=?,destination=?,order_date=?,required_delivery_date=?,currency=?,comments=?,exoneration_reference=?,general_discount_type=?,general_discount=?,gross=?,item_discount=?,allocated_discount=?,base_15=?,base_18=?,exempt=?,exonerated=?,isv_15=?,isv_18=?,total=?,products_investment=?,additional_expenses=?,investment_total=?,calculator_version=?,version=version+1,updated_by=?,team_id=?,updated_at=UTC_TIMESTAMP(6) WHERE id=? AND version=?`,
			o.OrderNumber, nullable(o.Customer.ID), o.CustomerVersion, o.Customer.Name, o.Customer.RTN, o.Customer.Contact, o.Customer.Phone, o.Customer.Email, o.Customer.Address,
			o.Destination, o.OrderDate, nullableDate(o.RequiredDeliveryDate), o.Currency, o.Comments, o.Exoneration, o.DiscountType, o.Discount,
			c.Gross, c.ItemDiscount, c.GeneralDiscount, c.Base15, c.Base18, c.Exempt, c.Exonerated, c.ISV15, c.ISV18, c.Total,
			c.ProductsInvestment, c.AdditionalExpenses, c.InvestmentTotal, c.CalculatorVersion, a.UserID, nullable(o.TeamID), o.ID, o.Version-1,
		)); e != nil {
			return nil, orderDBError(e)
		}
		if _, e := tx.ExecContext(ctx, `DELETE FROM customer_purchase_order_items WHERE purchase_order_id=?`, o.ID); e != nil {
			return nil, e
		}
	}

	for i, x := range o.Items {
		if _, e := tx.ExecContext(ctx, `INSERT INTO customer_purchase_order_items(purchase_order_id,position,description,unit,quantity,unit_sale_price,unit_cost,discount_type,discount,tax_type,supplier_id,supplier_version,supplier_name_snapshot,status,tracking,comments,unit_discount,gross,item_discount,allocated_discount,base,line_investment,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))`,
			o.ID, i, x.Description, x.Unit, x.Quantity, x.UnitSalePrice, x.UnitCost, x.DiscountType, x.Discount, x.TaxType, nullable(x.SupplierID), x.SupplierVersion, x.SupplierName, x.Status, x.Tracking, x.Comments,
			x.UnitDiscount, x.Gross, x.ItemDiscount, x.AllocatedDiscount, x.Base, x.LineInvestment,
		); e != nil {
			return nil, orderDBError(e)
		}
	}
	if e := s.Core.Audit(ctx, tx, a, topic, "customer_purchase_orders", o.ID, "success", map[string]any{"version": fmt.Sprint(o.Version), "items": len(o.Items)}); e != nil {
		return nil, e
	}
	if e := s.Core.EmitEvent(ctx, tx, a, topic, "customer_purchase_orders", o.ID, o.Version); e != nil {
		return nil, e
	}
	return map[string]string{"id": o.ID, "version": fmt.Sprint(o.Version)}, nil
}
