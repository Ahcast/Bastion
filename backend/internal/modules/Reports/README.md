# Reports

Fuente: [Documento maestro vigente](../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 19. Módulo Reportes

Reportes:

- ventas;
- compras;
- margen;
- clientes;
- proveedores;
- procesos;
- cotizaciones;
- facturas;
- gastos;
- RFQ;
- AI;
- operaciones.

Ejemplos:

- margen por cliente;
- margen por institución;
- procesos ganados/perdidos;
- cotizaciones por usuario;
- tiempo promedio de cotización;
- proveedor más competitivo;
- cuentas por cobrar;
- gastos por categoría;
- rentabilidad por OC.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../core/README.md) y cumple el [contrato modular](../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan los mismos servicios de aplicación/Services y políticas de autorización.

