# PurchaseOrders

Fuente: [Documento maestro v0.5](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 16. Módulo Órdenes de Compra

En el menú inicial significa:

> órdenes de compra emitidas por clientes después de aprobar una cotización.

Entidad conceptual:

`customer_purchase_orders`

Campos:

- cliente;
- número de OC;
- cotización;
- fecha;
- items;
- monto;
- lugar de entrega;
- fecha de entrega;
- documentos;
- estado.

Estados:

```text
Recibida
Confirmada
Comprando
En tránsito
Recibido
Preparando entrega
Entregado
Facturado
Cerrado
```

A futuro debe existir separado:

`supplier_purchase_orders`

para órdenes emitidas por Castillo IT a proveedores.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../../Core/README.md) y cumple el [contrato modular](../../../../../docs/contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan las mismas Actions/Services y Policies.
