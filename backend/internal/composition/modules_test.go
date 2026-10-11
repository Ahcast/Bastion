package composition

import (
	"io"
	"log/slog"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
)

func compositionFixture(t *testing.T, purchaseOrders string) (*core.Service, *registry.Registry) {
	t.Helper()
	for name, value := range map[string]string{
		"BASTION_CUSTOMERS_ENABLED":       "false",
		"BASTION_SUPPLIERS_ENABLED":       "false",
		"BASTION_PROCESSES_ENABLED":       "false",
		"BASTION_WORKFLOW_ENABLED":        "false",
		"BASTION_QUOTES_ENABLED":          "false",
		"BASTION_PURCHASE_ORDERS_ENABLED": purchaseOrders,
	} {
		t.Setenv(name, value)
	}

	s := &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
	r := registry.New()
	if err := r.Register(registry.Module{ID: "core", Label: "Plataforma", Version: "0.1.0", Enabled: true, Permissions: core.Permissions}); err != nil {
		t.Fatal(err)
	}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	if err := RegisterFiles(s, r, logger, nil); err != nil {
		t.Fatal(err)
	}
	return s, r
}

func moduleByID(t *testing.T, r *registry.Registry, id string) registry.Module {
	t.Helper()
	for _, m := range r.List() {
		if m.ID == id {
			return m
		}
	}
	t.Fatalf("module %s not registered", id)
	return registry.Module{}
}

func hasPermission(list []string, permission string) bool {
	for _, value := range list {
		if value == permission {
			return true
		}
	}
	return false
}

func TestRegisterPurchaseOrdersWithoutCatalogModules(t *testing.T) {
	s, r := compositionFixture(t, "true")
	m := moduleByID(t, r, "purchase-orders")
	if !m.Enabled {
		t.Fatal("purchase-orders must be enabled")
	}
	if len(m.Dependencies) != 1 || m.Dependencies[0] != "core" {
		t.Fatalf("unexpected dependencies: %#v", m.Dependencies)
	}
	if m.Routes == nil {
		t.Fatal("purchase-orders routes were not composed")
	}
	for _, permission := range purchaseorders.Permissions {
		if !hasPermission(s.PermissionCatalog(), permission) {
			t.Fatalf("permission %s was not registered", permission)
		}
	}
}

func TestRegisterPurchaseOrdersCanBeDisabled(t *testing.T) {
	s, r := compositionFixture(t, "false")
	m := moduleByID(t, r, "purchase-orders")
	if m.Enabled {
		t.Fatal("purchase-orders must be disabled")
	}
	for _, permission := range purchaseorders.Permissions {
		if hasPermission(s.PermissionCatalog(), permission) {
			t.Fatalf("disabled module registered permission %s", permission)
		}
	}
}
