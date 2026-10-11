package purchaseorders

const (
	StatusReceived          = "received"
	StatusConfirmed         = "confirmed"
	StatusPurchasing        = "purchasing"
	StatusInTransit         = "in_transit"
	StatusGoodsReceived     = "goods_received"
	StatusPreparingDelivery = "preparing_delivery"
	StatusDelivered         = "delivered"
	StatusInvoiced          = "invoiced"
	StatusClosed            = "closed"

	ItemStatusPending   = "pending"
	ItemStatusPurchased = "purchased"
	ItemStatusShipped   = "shipped"
	ItemStatusReceived  = "received"
	ItemStatusDelivered = "delivered"
)

var canonicalOrderStatuses = map[string]struct{}{
	StatusReceived:          {},
	StatusConfirmed:         {},
	StatusPurchasing:        {},
	StatusInTransit:         {},
	StatusGoodsReceived:     {},
	StatusPreparingDelivery: {},
	StatusDelivered:         {},
	StatusInvoiced:          {},
	StatusClosed:            {},
}

var unit2OrderNext = map[string]string{
	StatusReceived:          StatusConfirmed,
	StatusConfirmed:         StatusPurchasing,
	StatusPurchasing:        StatusInTransit,
	StatusInTransit:         StatusGoodsReceived,
	StatusGoodsReceived:     StatusPreparingDelivery,
	StatusPreparingDelivery: StatusDelivered,
}

var canonicalItemStatuses = map[string]struct{}{
	ItemStatusPending:   {},
	ItemStatusPurchased: {},
	ItemStatusShipped:   {},
	ItemStatusReceived:  {},
	ItemStatusDelivered: {},
}

var unit2ItemNext = map[string]string{
	ItemStatusPending:   ItemStatusPurchased,
	ItemStatusPurchased: ItemStatusShipped,
	ItemStatusShipped:   ItemStatusReceived,
	ItemStatusReceived:  ItemStatusDelivered,
}

// IsOrderStatus reports whether status belongs to the canonical customer-PO
// vocabulary. Invoiced and closed are canonical but intentionally have no Unit 2
// transition edge because their owner contracts do not exist yet.
func IsOrderStatus(status string) bool {
	_, ok := canonicalOrderStatuses[status]
	return ok
}

// NextOrderStatus returns the only state that Unit 2 permits after status.
// Reserved future states and delivered have no next state here.
func NextOrderStatus(status string) (string, bool) {
	next, ok := unit2OrderNext[status]
	return next, ok
}

// CanOrderTransition enforces the conservative Unit 2 graph: exactly one
// adjacent forward edge, never a skip, reversal, invoicing or close.
func CanOrderTransition(from, to string) bool {
	next, ok := NextOrderStatus(from)
	return ok && next == to
}

func IsItemStatus(status string) bool {
	_, ok := canonicalItemStatuses[status]
	return ok
}

func NextItemStatus(status string) (string, bool) {
	next, ok := unit2ItemNext[status]
	return next, ok
}

func CanItemTransition(from, to string) bool {
	next, ok := NextItemStatus(from)
	return ok && next == to
}
