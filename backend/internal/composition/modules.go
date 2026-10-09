// Package composition is the only package that wires Core and business modules.
package composition

import (
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	"github.com/Ahcast/Bastion/backend/internal/modules/customers"
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
	return r.Validate()
}
