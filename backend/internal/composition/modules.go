// Package composition is the only package that wires Core and business modules.
package composition

import (
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
	"io"
	"log/slog"
	"os"
)

func Register(s *core.Service, r *registry.Registry, logger *slog.Logger) error {
	return RegisterFiles(s, r, logger, nil)
}
func RegisterFiles(s *core.Service, r *registry.Registry, logger *slog.Logger, objects core.Objects) error {
	enabled := true
	if v := os.Getenv("BASTION_CUSTOMERS_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid customers enable flag")
		}
		enabled = v == "true"
	}
	service := &customers.Service{Core: s}
	if enabled {
		s.RegisterPermissions(customers.Permissions, []string{"clientes.view"})
		s.DocumentPolicies["customers"] = service.DocumentPolicy
	} else {
		delete(s.DocumentPolicies, "customers")
	}
	if e := r.Register(service.Module(logger, enabled)); e != nil {
		return e
	}
	supplierEnabled := true
	if v := os.Getenv("BASTION_SUPPLIERS_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid suppliers enable flag")
		}
		supplierEnabled = v == "true"
	}
	supplierService := &suppliers.Service{Core: s}
	if supplierEnabled {
		s.RegisterPermissions(suppliers.Permissions, []string{"proveedores.view"})
		s.DocumentPolicies["suppliers"] = supplierService.DocumentPolicy
	} else {
		delete(s.DocumentPolicies, "suppliers")
	}
	if e := r.Register(supplierService.Module(logger, supplierEnabled)); e != nil {
		return e
	}
	processEnabled := true
	if v := os.Getenv("BASTION_PROCESSES_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid processes enable flag")
		}
		processEnabled = v == "true"
	}
	processService := &processes.Service{Core: s}
	if processEnabled {
		s.RegisterPermissions(processes.Permissions, []string{"procesos.view"})
		s.RegisterServiceWrites(processes.CrawlerPermissions)
	}
	if e := r.Register(processService.Module(logger, processEnabled)); e != nil {
		return e
	}

	workflowEnabled := true
	if v := os.Getenv("BASTION_WORKFLOW_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid workflow enable flag")
		}
		workflowEnabled = v == "true"
	}
	approvalService := &workflow.Service{Core: s}
	if e := r.Register(approvalService.Module(workflowEnabled)); e != nil {
		return e
	}
	quoteEnabled := true
	if v := os.Getenv("BASTION_QUOTES_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid quotes enable flag")
		}
		quoteEnabled = v == "true"
	}
	quoteService := &quotes.Service{Core: s}
	if workflowEnabled {
		quoteService.Approvals = approvalService
	}
	if quoteEnabled && objects != nil {
		store, ok := objects.(quotes.PDFStore)
		if !ok {
			return fmt.Errorf("generated PDF storage unavailable")
		}
		quoteService.Store = store
	}
	for name, dst := range map[string]*[]byte{"BASTION_PDF_LOGO": &quoteService.Logo, "BASTION_PDF_STAMP": &quoteService.Stamp} {
		if path := os.Getenv(name); quoteEnabled && path != "" {
			f, e := os.Open(path)
			if e != nil {
				return fmt.Errorf("PDF graphic unavailable")
			}
			data, e := io.ReadAll(io.LimitReader(f, 1024*1024+1))
			f.Close()
			if e != nil || quotes.ValidateGraphic(data) != nil {
				return fmt.Errorf("invalid PDF graphic")
			}
			*dst = data
		}
	}

	if enabled {
		quoteService.Customers = service
	}
	if processEnabled {
		quoteService.Processes = processService
	}
	if quoteEnabled {
		s.RegisterPermissions(quotes.Permissions, []string{"cotizaciones.view"})
		s.DocumentPolicies["quotes"] = quoteService.DocumentPolicy
	}
	if !quoteEnabled {
		delete(s.DocumentPolicies, "quotes")
	}
	if e := r.Register(quoteService.ModuleFiles(logger, quoteEnabled, objects)); e != nil {
		return e
	}

	purchaseOrderEnabled := true
	if v := os.Getenv("BASTION_PURCHASE_ORDERS_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid purchase orders enable flag")
		}
		purchaseOrderEnabled = v == "true"
	}
	purchaseOrderService := &purchaseorders.Service{Core: s}
	if enabled {
		purchaseOrderService.Customers = service
	}
	if supplierEnabled {
		purchaseOrderService.Suppliers = supplierService
	}
	if purchaseOrderEnabled && quoteEnabled {
		quoteService.PurchaseOrders = purchaseOrderService
	}
	if purchaseOrderEnabled {
		s.RegisterPermissions(purchaseorders.Permissions, []string{"ordenes.view"})
		s.DocumentPolicies["purchase_orders"] = purchaseOrderService.DocumentPolicy
	}
	if !purchaseOrderEnabled {
		delete(s.DocumentPolicies, "purchase_orders")
	}
	if e := r.Register(purchaseOrderService.Module(logger, purchaseOrderEnabled)); e != nil {
		return e
	}
	return r.Validate()
}
