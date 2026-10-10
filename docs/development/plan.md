# Plan ejecutable del proyecto

Core de plataforma y base implementados: ver [checkpoint operativo](../ESTADO_DESARROLLO.md), [estado histórico](status.md) y [runbook](core-runbook.md). Los criterios de infraestructura externa permanecen pendientes aunque las pruebas locales/CI pasen. Clientes, Proveedores y la base de Procesos/receptor crawler están cerrados. Cotizaciones tiene implementados borradores/cálculo/conversión desde Procesos, PDF/adjuntos, estados/revisiones, aprobación humana, emisión segura de PDF, correo comercial durable y bulk trazable. La unidad activa siguiente es el contrato e implementación de FX fechado; no iniciar OC todavía. Consultar el checkpoint para evidencia vigente y continuar una unidad por vez.

El agente implementa cada fase hasta su aceptación, actualiza el checkpoint/estado y continúa. No sustituir implementación por plan nuevo. Las fases pueden desarrollarse con datos sintéticos y MySQL 5.7 local/CI de prueba; los bloqueos de infraestructura se verifican antes de producción, sin migrar la BD a AWS.

| Fase | Trabajo y entregables | Criterio de aceptación |
| --- | --- | --- |
| 0. Base | Inicializar Go/Angular, lockfiles, configuración/env.example, comandos de desarrollo, SQL migrations runner, CI; inventario del crawler | Build reproducible; pruebas arrancan; secretos ausentes; versión de esquema y health/readiness definidos |
| 1. Core | Usuarios/invitaciones, sesiones, TOTP/recovery, RBAC configurable, settings/SMTP, auditoría, API/errores, cliente Angular, navegación y Configuración | Login→MFA→me→logout; sesión parcial denegada; permisos por registro; revocación; CORS/CSRF real en navegador; SMTP solo fake local hasta autorización |
| 2. Servicios compartidos | Archivos privados/metadatos/S3, outbox/jobs/retries, idempotencia, contexto/correlación, límites y registry módulos | Doble solicitud/concurrencia no duplica; commit+outbox atómico; crash/retry recuperable; archivo no publicado antes de verificar; acceso ajeno denegado |
| 3. Datos base y Procesos | Clientes, proveedores, procesos/items/versiones, receptor crawler/ACK, filtros, Inicio; conservar CLI Python | Pruebas crawler existentes; importar/reimportar lote; observación antigua/parcial no borra; ACK solo tras commit; listados autorizados y cliente snapshot |
| 4. Comercial | Quotes, cálculo decimal/versiones/adjuntos/PDF/correo programado; OC de clientes; Invoices/payments; Expenses | Ejemplos 15/18/exento/exonerado y descuento unitario/general; control de versión; conversiones transaccionales/idempotentes; saldos consistentes; correo incierto no reenvía a ciegas |
| 5. Reportes y colaboración | Reportes/exportaciones, documentos/versiones/folders, colaboración y workflows/aprobaciones configurables | Filtros/permisos idénticos a dominio; totales conciliados; documentos privados; autorización por acción/hash/version/vigencia; sin OCR local |
| 6. API/MCP IA | SDK MCP, autenticación limitada/delegación conforme, tools mínimo, AI tasks/runs/leases y score/research/RFQ/pricing/artifacts | Mismas reglas REST/MCP; lease vencido/resultado tardío rechazado; schema/permissions; prueba cliente local; Work real solo si disponible, no invocar Plus como API |
| 7. IA y UI | Pipeline/skills versionadas, progresos por ítem, RAG S3/MySQL, revisión humana y selección hacia Quotes | No repetir score completado; rezagados recuperables; evidencia/ausentes explícitos; cotización humana no sobrescrita; casos de evaluación sintéticos; umbrales no aprobados permanecen configurables y automatización off |
| 8. Operación | Backup/restore, deploy scripts frontend/backend, observabilidad, incidentes, límites y smoke tests | Restore probado; rollback código/esquema documentado; memoria crawler+backend medida; MySQL TLS/DNS/SMTP/S3 validados en entorno autorizado; no claims de producción por tests locales |

## Secuencia obligatoria de módulos

Clientes → Proveedores → Procesos/receptor crawler → Cotizaciones → Órdenes de Compra de clientes → Facturas/pagos → Gastos → Reportes → Colaboración. Workflow/aprobaciones e IA se integran según dependencias, sin entradas adicionales de menú. Antes de avanzar, cerrar pruebas unitarias, MySQL/integridad, autorización, UI/navegador, regresión y CI de la unidad actual; registrar evidencia y límites externos en el checkpoint/status. No escribir el siguiente módulo mientras haya pruebas pendientes/fallidas o reglas comerciales indispensables sin contrato. Las vistas cruzadas se incorporan cuando ambos propietarios existan, sin inventar datos ni declarar integración anticipada.

## Antes de implementar cada módulo

1. Leer Core, modelo, API y spec funcional propietaria.
2. Definir migraciones/índices, entidades/DTO, estados/transiciones, permisos, eventos/jobs y errores. No copiar toda la documentación al código.
3. Implementar servicios y pruebas de invariantes; handlers REST/MCP finos; Angular consume contratos.
4. Verificar casos de fallo reales: duplicados, payload/version/lease inválidos, acceso denegado, caída antes/después del commit.
5. Actualizar OpenAPI, docs solo si cambia contrato, y status/checkpoint con comandos/resultados. Commit en main.

## Definición de terminado

Función completa, error explícito y manejo de fallos; build y pruebas relevantes; migración reproducible; autorización/auditoría; UI española; documentos/snapshots consistentes; estado actualizado. Sin stubs productivos, sin promesas de capacidades de terceros ni tests que solo repitan implementación.

## Bloqueos externos acotados

MySQL remoto TLS o túnel SSH verificado, acceso y soporte Percona; DNS/certificados; S3/IAM; SMTP/IMAP; fiscalidad SAR/series/plantillas reales; Work/Grokbot y skills comerciales. Registrar evidencia ausente. Mantener escenarios locales de prueba y continuar módulos independientes. Feature flags dejan conexiones/envíos externos deshabilitados hasta configurar y validar.

No implementar integración pagada de inferencia como solución automática a un bloqueo Work. RPO/RTO, retención y reglas de scoring/envío se configuran sin inventar valores aprobados.

## Cotizaciones — continuidad vigente

Correo comercial y bulk dejaron de ser pendientes: ambos están implementados y validados en CI. Bulk durable está certificado en el checkpoint con migración 0013, REST, Angular y Playwright real.

La siguiente unidad es **FX fechado**. El sistema permite cotizaciones explícitas en HNL o USD, pero no conversión implícita. Antes de programar FX se debe definir de forma normativa la fuente/tipo de tasa, fecha efectiva, snapshot/historial, precisión/redondeo, autoridad de modificación, efecto sobre versiones/aprobaciones/emisiones y presentación UI/PDF/exportación. No inventar esos valores en código.

Las conversiones Quotes→OC/Factura se integran con los servicios propietarios cuando esos módulos existan; Quotes no escribe sus tablas directamente. Esto no autoriza a saltar a OC mientras FX y la aceptación restante de Cotizaciones no estén cerrados.
