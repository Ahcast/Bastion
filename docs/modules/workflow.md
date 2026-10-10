# Workflow


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

Workflow administra aprobaciones; Core aporta identidad, políticas de autorización y auditoría.

## 24.1 Análisis y borradores
Con permiso puede analizar, clasificar, investigar, buscar precios/proveedores, preparar candidatos, documentos y borradores de RFQ. Preparar una solicitud no autoriza su envío.

## 24.2 Escrituras y correo
Cambios comerciales requieren permiso/Policy del módulo. Envíos RFQ o follow-up requieren aprobación por defecto o regla previa explícitamente autorizada con propósito, destinatarios y límites. El correo al cliente y acciones sensibles siguen los controles Core/Workflow.

## 24.3 Acciones críticas
Presentar ofertas, emitir órdenes a proveedores, comprometer dinero y autorizar pagos requieren decisión humana. Aprobación vinculada a acción, entidad, versión/hash y vigencia; cambios relevantes invalidan la aprobación. Reevaluar antes de ejecutar y evitar duplicados.

---


## Integración con Core

Este módulo conserva su alcance funcional. Consume [Core](../architecture/core.md) y cumple el [contrato modular](../contracts/modules.md). Sus endpoints REST y herramientas MCP reutilizan los mismos servicios de aplicación/Services y políticas de autorización.



## Base implementada

internal/modules/workflow evalúa grafos explícitos con denegación por defecto. Quotes define su grafo preliminar y aplica versión, permisos, auditoría/historial/idempotencia en su propia TX. El ledger approvals/approval_events implementa solicitudes/decisiones/revocación/invalidation/consumo transaccional, vinculado por el propietario a acción, entidad, versión, hash y vigencia. Quotes incorpora esa base de aprobación humana con permisos específicos y separación entre creador/solicitante y aprobador. Revalidación de cuenta/MFA/grants actuales antes de consumo; no reutiliza una sesión histórica. Ver [contrato de Cotizaciones](../contracts/quotes.md). Definiciones editables y workflows de los módulos futuros todavía no están implementados; no se habilitan envíos por alcanzar review o approved.
