// Package composition is the only package that wires Core and business modules.
package composition

import (
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
	"github.com/Ahcast/Bastion/backend/internal/modules/processes"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/Ahcast/Bastion/backend/internal/modules/suppliers"
	"log/slog"
	"os"
)

func Register(s *core.Service, r *registry.Registry, logger *slog.Logger) error {
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

	quoteEnabled := true
	if v := os.Getenv("BASTION_QUOTES_ENABLED"); v != "" {
		if v != "true" && v != "false" {
			return fmt.Errorf("invalid quotes enable flag")
		}
		quoteEnabled = v == "true"
	}
	quoteService := &quotes.Service{Core: s}
	if enabled {
		quoteService.Customers = service
	}
	if processEnabled {
		quoteService.Processes = processService
	}
	if quoteEnabled {
		s.RegisterPermissions(quotes.Permissions, []string{"cotizaciones.view"})
	}
	if e := r.Register(quoteService.Module(logger, quoteEnabled)); e != nil {
		return e
	}
	return r.Validate()
}
