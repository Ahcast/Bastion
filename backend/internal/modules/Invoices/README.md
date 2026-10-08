# Invoices

Fuente: [Documento maestro vigente](../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 17. Módulo Facturas

Relacionable con:

- cliente;
- cotización;
- orden de compra;
- items;
- impuestos;
- pagos.

Estados:

```text
Borrador
Emitida
Entregada
Pendiente
Parcialmente pagada
Pagada
Vencida
Anulada
```

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../core/README.md) y cumple el [contrato modular](../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan los mismos servicios de aplicación/Services y políticas de autorización.

