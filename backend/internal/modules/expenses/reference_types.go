package expenses

import (
	"context"
	"database/sql"
	"sync"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
)

type ExpenseReferenceSource interface {
	ExpenseReferenceInTransaction(context.Context, *sql.Tx, core.Actor, string) (string, uint64, error)
}

type ReferenceSources struct {
	Processes      ExpenseReferenceSource
	Quotes         ExpenseReferenceSource
	PurchaseOrders ExpenseReferenceSource
	Invoices       ExpenseReferenceSource
}

var expenseReferenceSources sync.Map

func ConfigureReferenceSources(service *Service, sources ReferenceSources) {
	if service != nil {
		expenseReferenceSources.Store(service, sources)
	}
}

func (s *Service) referenceSources() ReferenceSources {
	v, ok := expenseReferenceSources.Load(s)
	if !ok {
		return ReferenceSources{}
	}
	return v.(ReferenceSources)
}

type ReferenceInput struct {
	EntityType    string `json:"entity_type"`
	EntityID      string `json:"entity_id"`
	EntityVersion uint64 `json:"entity_version,string"`
}

type SetReferencesInput struct {
	Version    uint64           `json:"version,string"`
	Reason     string           `json:"reason"`
	References []ReferenceInput `json:"references"`
}

type Reference struct {
	EntityType    string    `json:"entity_type"`
	EntityID      string    `json:"entity_id"`
	EntityVersion uint64    `json:"entity_version,string"`
	Label         string    `json:"label"`
	LinkedBy      string    `json:"linked_by"`
	LinkedAt      time.Time `json:"linked_at"`
}

var referenceOrder = []string{"processes", "quotes", "purchase_orders", "invoices"}

func referenceSource(s ReferenceSources, kind string) ExpenseReferenceSource {
	switch kind {
	case "processes":
		return s.Processes
	case "quotes":
		return s.Quotes
	case "purchase_orders":
		return s.PurchaseOrders
	case "invoices":
		return s.Invoices
	default:
		return nil
	}
}
