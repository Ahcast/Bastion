# Expenses

Fuente: [Documento maestro vigente](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 18. Módulo Gastos

Datos:

- proveedor;
- categoría;
- fecha;
- monto;
- moneda;
- impuesto;
- proceso;
- cotización;
- OC;
- factura soporte;
- comprobante.

Categorías posibles:

```text
Compra producto
Flete
Aduana
Impuestos
Transporte
Viáticos
Software
Servicios
Publicidad
Oficina
Otros
```

Objetivo:

- rentabilidad real por proceso;
- rentabilidad por cotización;
- rentabilidad por OC;
- análisis financiero.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../Core/README.md) y cumple el [contrato modular](../../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan las mismas Actions/Services y Policies.
