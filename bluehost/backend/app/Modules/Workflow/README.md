# Workflow

Fuente: [Documento maestro v0.5](../../../../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 22. Workflow Engine

Debe ser transversal.

Entidades:

- procesos;
- cotizaciones;
- OC;
- facturas;
- RFQ;
- gastos;
- tareas.

Ejemplo cotización:

```text
Borrador
   ↓
Costeo
   ↓
Revisión
   ↓
Aprobación
   ↓
Enviada
```

El sistema debe permitir extender o redefinir workflows.

---


# 23. Event-driven interno

Ejemplos:

`QuoteApproved`

puede disparar:

- audit;
- notifications;
- dashboard refresh;
- AI;
- workflow.

`CustomerPurchaseOrderReceived`

puede disparar:

- crear workflow;
- crear tareas;
- notificar responsable;
- iniciar procurement;
- actualizar dashboard.

Objetivo:

- desacoplar módulos;
- permitir funciones futuras.

---


# 24. Approval Engine

Separación:

## 24.1 Autónomo

AI puede:

- analizar;
- clasificar;
- investigar;
- buscar precios;
- buscar proveedores;
- crear candidatos;
- generar documentos;
- solicitar cotizaciones;
- hacer follow-up.

## 24.2 Según rol

- crear factura;
- editar cotización;
- enviar email al cliente;
- modificar ciertos registros.

## 24.3 Requiere aprobación

- presentar oferta;
- emitir OC a proveedor;
- comprometer dinero;
- autorizar pago;
- acciones críticas;
- acciones irreversibles.

---

