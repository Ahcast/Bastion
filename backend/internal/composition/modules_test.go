package composition

import (
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
)

func TestQuotePurchaseOrderConversionRequiresBothOwners(t *testing.T) {
	for _, enabled := range []bool{false, true} {
		t.Run(fmt.Sprint(enabled), func(t *testing.T) {
			for _, name := range []string{"BASTION_CUSTOMERS_ENABLED", "BASTION_SUPPLIERS_ENABLED", "BASTION_PROCESSES_ENABLED", "BASTION_WORKFLOW_ENABLED"} {
				t.Setenv(name, "false")
			}
			t.Setenv("BASTION_QUOTES_ENABLED", "true")
			t.Setenv("BASTION_PURCHASE_ORDERS_ENABLED", fmt.Sprint(enabled))
			s := &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
			r := registry.New()
			if e := r.Register(registry.Module{ID: "core", Label: "Core", Version: "0.1.0", Enabled: true}); e != nil {
				t.Fatal(e)
			}
			if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e != nil {
				t.Fatal(e)
			}
			mux := http.NewServeMux()
			moduleByID(t, r, "quotes").Routes(mux)
			_, pattern := mux.Handler(httptest.NewRequest("POST", "/api/v1/quotes/1/convert-to-purchase-order", nil))
			if (pattern != "") != enabled {
				t.Fatal("conversion route availability", pattern, enabled)
			}
		})
	}
}

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

func TestRegisterCollaborationCanBeDisabledWithoutLeavingFilePolicy(t *testing.T) {
	for _, enabled := range []bool{true, false} {
		t.Run(fmt.Sprint(enabled), func(t *testing.T) {
			t.Setenv("BASTION_COLLABORATION_ENABLED", fmt.Sprint(enabled))
			s, r := compositionFixture(t, "false")
			m := moduleByID(t, r, "collaboration")
			if m.Enabled != enabled || m.Routes == nil {
				t.Fatalf("module registration: %#v", m)
			}
			if _, ok := s.DocumentPolicies["collaboration"]; ok != enabled {
				t.Fatal("collaboration document policy did not follow module switch")
			}
			if _, ok := s.DocumentBindingPolicies["collaboration"]; ok != enabled {
				t.Fatal("collaboration binding policy did not follow module switch")
			}
			if _, ok := s.DocumentVersionPolicies["collaboration"]; ok != enabled {
				t.Fatal("collaboration version policy did not follow module switch")
			}
			if _, ok := s.DocumentTrashPolicies["collaboration"]; ok != enabled {
				t.Fatal("collaboration trash policy did not follow module switch")
			}
			mux := http.NewServeMux()
			r.Mount(mux)
			_, pattern := mux.Handler(httptest.NewRequest("GET", "/api/v1/collaboration", nil))
			if (pattern != "") != enabled {
				t.Fatal("route availability", pattern, enabled)
			}
			for _, permission := range []string{"colaboracion.view", "colaboracion.upload", "colaboracion.organize", "colaboracion.delete", "colaboracion.comment", "colaboracion.moderate"} {
				if hasPermission(s.PermissionCatalog(), permission) != enabled {
					t.Fatal("permission catalog switch", permission, enabled)
				}
			}
		})
	}
	t.Setenv("BASTION_COLLABORATION_ENABLED", "invalid")
	s := &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
	r := registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e == nil {
		t.Fatal("invalid module flag accepted")
	}
}

func TestPurchaseOrderDocumentPolicyFollowsModuleFlag(t *testing.T) {
	s, _ := compositionFixture(t, "true")
	if s.DocumentPolicies["purchase_orders"] == nil {
		t.Fatal("enabled owner policy missing")
	}
	s, _ = compositionFixture(t, "false")
	if s.DocumentPolicies["purchase_orders"] != nil {
		t.Fatal("disabled owner policy remains")
	}
}

func TestInvoicesDraftModuleFlagAndExplicitPermissions(t *testing.T) {
	for _, enabled := range []bool{false, true} {
		t.Run(fmt.Sprint(enabled), func(t *testing.T) {
			t.Setenv("BASTION_INVOICES_ENABLED", fmt.Sprint(enabled))
			s, r := compositionFixture(t, "false")
			m := moduleByID(t, r, "invoices")
			if m.Enabled != enabled || len(m.Dependencies) != 1 || m.Dependencies[0] != "core" {
				t.Fatal(m)
			}
			for _, permission := range []string{"facturas.view", "facturas.create", "facturas.update", "facturas.fiscal.read", "facturas.fiscal.manage", "facturas.emit", "facturas.payment", "facturas.payment.void"} {
				if hasPermission(s.PermissionCatalog(), permission) != enabled {
					t.Fatal(permission, enabled)
				}
			}
		})
	}
	t.Setenv("BASTION_INVOICES_ENABLED", "invalid")
	s := &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
	r := registry.New()
	if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e == nil || e.Error() != "invalid invoices enable flag" {
		t.Fatal("invalid invoice flag", e)
	}
}

func TestExpensesModuleFlagAndOptionalSupplierCatalog(t *testing.T) {
	for _, enabled := range []bool{false, true} {
		t.Setenv("BASTION_EXPENSES_ENABLED", fmt.Sprint(enabled))
		s, r := compositionFixture(t, "false")
		m := moduleByID(t, r, "expenses")
		if m.Enabled != enabled || len(m.Dependencies) != 1 || m.Dependencies[0] != "core" {
			t.Fatal(m)
		}
		for _, p := range []string{"gastos.view", "gastos.create", "gastos.update", "gastos.delete", "gastos.restore"} {
			if hasPermission(s.PermissionCatalog(), p) != enabled {
				t.Fatal(p)
			}
		}
	}
	t.Setenv("BASTION_EXPENSES_ENABLED", "invalid")
	s := &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
	r := registry.New()
	if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e == nil || e.Error() != "invalid expenses enable flag" {
		t.Fatal(e)
	}
}

func TestReportsCompositionFlagsAndSourceRoutes(t *testing.T) {
	for _, enabled := range []string{"true", "false"} {
		t.Run(enabled, func(t *testing.T) {
			t.Setenv("BASTION_REPORTS_ENABLED", enabled)
			t.Setenv("BASTION_INVOICES_ENABLED", "false")
			t.Setenv("BASTION_EXPENSES_ENABLED", "true")
			s, r := compositionFixture(t, "false")
			m := moduleByID(t, r, "reports")
			if m.Enabled != (enabled == "true") {
				t.Fatal(m)
			}
			if hasPermission(s.PermissionCatalog(), "reportes.view") != (enabled == "true") {
				t.Fatal("report permission flag")
			}
			if enabled == "true" {
				mux := http.NewServeMux()
				m.Routes(mux)
				for _, kind := range []string{"invoices", "orders", "expenses"} {
					_, p := mux.Handler(httptest.NewRequest("GET", "/api/v1/reports/"+kind, nil))
					if (p != "") != (kind == "expenses") {
						t.Fatal(kind, p)
					}
				}
			}
		})
	}
	t.Setenv("BASTION_REPORTS_ENABLED", "invalid")
	s := &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
	r := registry.New()
	_ = r.Register(registry.Module{ID: "core", Enabled: true})
	if Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))) == nil {
		t.Fatal("invalid flag accepted")
	}
}

func TestAICompositionFollowsSourceAndExplicitSwitch(t *testing.T) {
	t.Setenv("BASTION_AI_ENABLED", "false")
	s, r := compositionFixture(t, "false")
	if moduleByID(t, r, "ai").Enabled || hasPermission(s.PermissionCatalog(), "ai.tasks.execute") {
		t.Fatal("disabled AI grants access")
	}
	t.Setenv("BASTION_AI_ENABLED", "true")
	r = registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e == nil {
		t.Fatal("AI allowed without Processes")
	}
	t.Setenv("BASTION_PROCESSES_ENABLED", "true")
	s = &core.Service{DocumentPolicies: map[string]core.DocumentPolicy{}}
	r = registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e != nil {
		t.Fatal(e)
	}
	if !moduleByID(t, r, "ai").Enabled || !hasPermission(s.PermissionCatalog(), "ai.tasks.execute") {
		t.Fatal("AI missing")
	}
	t.Setenv("BASTION_AI_ENABLED", "invalid")
	r = registry.New()
	_ = r.Register(registry.Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	if e := Register(s, r, slog.New(slog.NewTextHandler(io.Discard, nil))); e == nil || e.Error() != "invalid ai enable flag" {
		t.Fatal(e)
	}
}
