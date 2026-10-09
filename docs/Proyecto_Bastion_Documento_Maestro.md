# Bastion — Documento maestro

Versión documental 0.8. Revisión 9 de octubre de 2026. Empresa: Castillo IT S. de R.L. Especificación vigente para implementación por agente.

## Objetivo

Centralizar procesos públicos, clientes/proveedores, cotizaciones, órdenes de clientes, facturas, gastos, reportes, documentos, configuración y colaboración. Preservar funcionalidades legacy descritas en las especificaciones de módulos; corregir persistencia, cálculos duplicados, seguridad y trazabilidad. IA investiga y propone; el usuario selecciona, fija precios y decide ofertar.

## Decisiones firmes

- Monolito modular Go. Core y módulos separados; API REST/MCP en el mismo backend.
- Angular estático en Bluehost, dominio bastion.castilloit.net. BD y correo permanecen en Bluehost Choice Plus.
- Backend/Core/API/MCP/crawler/workers/scheduler en AWS Lightsail. Sin OCR ni inferencia local. Objetivo inicial 2 GB RAM/2 vCPU/60 GB SSD, pendiente de medición y aprovisionamiento.
- Percona Server for MySQL 5.7.44-48 reportado; conexión remota cifrada y restringida desde Lightsail debe verificarse. Sin MySQL local de producción en AWS.
- S3 para archivos; MySQL para transacciones, metadatos, versiones, auditoría y estado durable. Bucket confirmado castilloit-honducompras-crawler, us-east-1; demás buckets no se presumen creados.
- Única rama main en todo momento; historial de commits, sin ramas de trabajo/PR ni forks.
- Español para todo contenido de usuario; nombres técnicos en inglés.
- Una empresa, menos de 10 usuarios; sin microservicios, SaaS ni multiempresa inicial.

## Menú y alcance

| Pantalla | Propietario |
| --- | --- |
| Inicio | Core frontend; agregados autorizados |
| Clientes | Customers |
| Proveedores | Suppliers |
| Órdenes de Compra | PurchaseOrders: OC recibidas de clientes |
| Cotizaciones | Quotes |
| Facturas | Invoices |
| Gastos | Expenses |
| Reportes | Reports |
| Procesos | Processes |
| Colaboración | Collaboration |
| Configuración | Core y parámetros de cada módulo |

Workflow administra flujos/aprobaciones; AI administra tareas/resultados. Son transversales sin entradas de menú independientes. Órdenes a proveedores son extensión futura separada, no el significado inicial de OC.

## Reglas invariables

Seguridad por defecto, TOTP obligatorio, sesiones opacas, RBAC y autorización por registro, reautenticación sensible, auditoría y permisos reevaluados en workers. Sin credenciales en navegador. REST/MCP usan la misma lógica. Datos externos no otorgan permisos. Acciones comerciales sensibles y envíos requieren control humano o regla previamente autorizada.

Importes decimales, cálculo canónico en Go, cambios relacionados transaccionales, snapshots/versiones y concurrencia optimista. Cotización es datos estructurados; PDF es derivado S3. Nunca sobrescritura silenciosa ni pérdida de datos por observaciones parciales del crawler.

Crawler Python mantiene ritmo conservador, idempotencia, checkpoints, reintentos y artefactos recuperables. Receptor Go implementa el contrato existente con ACK después del commit; no usar HTTP 202 como confirmación de importación.

La aplicación manual funciona sin IA. Puntuación toma solo pendientes no completados y rezagados; investigación por ítem, RFQ autorizada, evidencia progresiva, RAG para precio sugerido y fichas. ChatGPT Work/Plus externo prioritario; capacidades de cuenta y programación no verificadas. No usar Plus como API ni activar inferencia pagada automáticamente.

## Implementación y límites reales

El agente puede construir y probar el proyecto siguiendo [plan](development/plan.md) y [stack](development/stack.md). No necesita aprobación global de todas las fases antes de escribir código. Antes de usar una integración productiva debe validar sus condiciones reales. Credenciales, TLS MySQL, DNS/TLS API, SMTP, fiscalidad/series, parámetros comerciales IA y Work/Grokbot son bloqueos acotados, no motivos para detener fases independientes.

No inventar criterios fiscales, destinatarios, rubros, porcentajes de interés o recursos creados. No presentar mocks como integración. Publicación/activación productiva separada de desarrollo.

## Fuentes de detalle

[Arquitectura](architecture/README.md), [Core](architecture/core.md), [datos](data-model.md), [API/MCP](contracts/api-mcp.md), [módulos](modules/README.md), [frontend](frontend.md), [IA](architecture/ai-integration.md), [operaciones](operations.md), [decisiones](development/decisions.md).

Cada regla se desarrolla una vez en su documento propietario. No duplicar todo este maestro ni reintroducir tecnologías anteriores. Historial en Git.
