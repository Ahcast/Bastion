package purchaseorders

import "testing"

func TestOrderStateVocabulary(t *testing.T) {
	canonical := []string{
		StatusReceived,
		StatusConfirmed,
		StatusPurchasing,
		StatusInTransit,
		StatusGoodsReceived,
		StatusPreparingDelivery,
		StatusDelivered,
		StatusInvoiced,
		StatusClosed,
	}
	for _, status := range canonical {
		if !IsOrderStatus(status) {
			t.Fatalf("canonical order status %q rejected", status)
		}
	}
	for _, legacy := range []string{"Pendiente", "Listo", "Entregado", "Facturado", "Converted", "pending", "ready"} {
		if IsOrderStatus(legacy) {
			t.Fatalf("legacy/noncanonical order status %q accepted", legacy)
		}
	}
}

func TestOrderStateGraphOnlyAllowsAdjacentUnit2Edges(t *testing.T) {
	allowed := [][2]string{
		{StatusReceived, StatusConfirmed},
		{StatusConfirmed, StatusPurchasing},
		{StatusPurchasing, StatusInTransit},
		{StatusInTransit, StatusGoodsReceived},
		{StatusGoodsReceived, StatusPreparingDelivery},
		{StatusPreparingDelivery, StatusDelivered},
	}
	for _, edge := range allowed {
		if !CanOrderTransition(edge[0], edge[1]) {
			t.Fatalf("allowed edge %q -> %q rejected", edge[0], edge[1])
		}
	}

	rejected := [][2]string{
		{StatusReceived, StatusPurchasing},
		{StatusConfirmed, StatusReceived},
		{StatusPreparingDelivery, StatusInvoiced},
		{StatusDelivered, StatusInvoiced},
		{StatusInvoiced, StatusClosed},
		{StatusClosed, StatusReceived},
		{StatusReceived, StatusReceived},
		{"Pendiente", StatusConfirmed},
		{StatusReceived, "Listo"},
	}
	for _, edge := range rejected {
		if CanOrderTransition(edge[0], edge[1]) {
			t.Fatalf("forbidden edge %q -> %q accepted", edge[0], edge[1])
		}
	}
}

func TestOrderNextStateStopsAtDeliveredAndReservedStates(t *testing.T) {
	next, ok := NextOrderStatus(StatusReceived)
	if !ok || next != StatusConfirmed {
		t.Fatalf("received next = %q, %v", next, ok)
	}
	for _, status := range []string{StatusDelivered, StatusInvoiced, StatusClosed, "unknown"} {
		if next, ok := NextOrderStatus(status); ok || next != "" {
			t.Fatalf("%q unexpectedly has Unit 2 next state %q", status, next)
		}
	}
}

func TestItemStateVocabularyAndGraph(t *testing.T) {
	canonical := []string{
		ItemStatusPending,
		ItemStatusPurchased,
		ItemStatusShipped,
		ItemStatusReceived,
		ItemStatusDelivered,
	}
	for _, status := range canonical {
		if !IsItemStatus(status) {
			t.Fatalf("canonical item status %q rejected", status)
		}
	}

	allowed := [][2]string{
		{ItemStatusPending, ItemStatusPurchased},
		{ItemStatusPurchased, ItemStatusShipped},
		{ItemStatusShipped, ItemStatusReceived},
		{ItemStatusReceived, ItemStatusDelivered},
	}
	for _, edge := range allowed {
		if !CanItemTransition(edge[0], edge[1]) {
			t.Fatalf("allowed item edge %q -> %q rejected", edge[0], edge[1])
		}
	}

	rejected := [][2]string{
		{ItemStatusPending, ItemStatusShipped},
		{ItemStatusPurchased, ItemStatusPending},
		{ItemStatusDelivered, ItemStatusPending},
		{ItemStatusPending, ItemStatusPending},
		{"Comprado", ItemStatusShipped},
	}
	for _, edge := range rejected {
		if CanItemTransition(edge[0], edge[1]) {
			t.Fatalf("forbidden item edge %q -> %q accepted", edge[0], edge[1])
		}
	}
}
