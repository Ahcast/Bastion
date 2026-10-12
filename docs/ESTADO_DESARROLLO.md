# Estado operativo de desarrollo — Bastion

Actualizado: 2026-10-12. Checkpoint vigente para reanudar. Arquitectura, contratos, decisiones y especificaciones siguen siendo normativos; el historial detallado está en [status](development/status.md) y Git.

## Regla de continuación

1. Trabajar únicamente sobre `main`; sincronizar HEAD y revisar el último CI antes de editar.
2. Leer AGENTS, maestro, arquitectura/stack, plan y contrato/especificación del propietario activo.
3. Corregir regresiones antes de avanzar. Certificar una unidad y actualizar estado antes de iniciar la siguiente.
4. Usar datos sintéticos y MySQL 5.7 real para integración. Sin DSN de prueba, tests omitidos no acreditan persistencia.
5. No desplegar, migrar BD productiva ni activar correo/integraciones sin autorización y validación externas.

## Estado vigente

| Propietario | Estado de desarrollo |
| --- | --- |
| Core | Implementado y certificado: identidad/MFA/RBAC, API/MCP interno, archivos, auditoría/idempotencia/outbox |
| Clientes y Proveedores | Registro y catálogo cerrados/certificados, scopes y snapshots históricos |
| Procesos/receptor crawler | Unidad base cerrada; CLI/ritmo/retries y contrato crawler preservados |
| Cotizaciones | Alcance HNL/USD cerrado: cálculo, estados/Workflow, PDF/adjuntos, correo/bulk, FX auxiliar y conversiones |
| Órdenes de Compra de clientes | Seis unidades cerradas: agregado, seguimiento/entrega, conversión, documentos, clonación, papelera/utilidad y aceptación; OC→Factura/cierre/reapertura integrados |
| Facturas/pagos | Siete unidades y aceptación cerradas: borradores, fiscal interno/emisión, pagos, conversiones, PDF, anulación conservadora y clonación |
| Gastos | Tres unidades y aceptación integral cerradas: registro/cálculo/proveedor/categorías, scopes/filtros/totales/duplicados/papelera, referencias y comprobantes privados |
| Reportes | **Siguiente módulo. Sin implementación propietaria todavía** |
| Colaboración, workflows configurables e IA | Fases posteriores según dependencias del plan; no confundir Workflow de aprobación ya integrado con configuración futura de flujos |
| Producción | No desplegada ni migrada; integraciones externas no acreditadas por CI |

## Cierre certificado de Gastos

Código `f3dd6121df2695cf89d9eb89c40dc28644f7a2c5`:

- [Core checks 38300556293](https://github.com/Ahcast/Bastion/actions/runs/38300556293): Go fmt/vet, `go test -race -count=1 ./...` sobre MySQL 5.7, builds server/worker/admin, Angular build/**143 pruebas** y navegador **18/18** contra API/Angular/BD reales de CI: success.
- [Documentación 38300556205](https://github.com/Ahcast/Bastion/actions/runs/38300556205) y [rama única 38300556380](https://github.com/Ahcast/Bastion/actions/runs/38300556380): success.
- Contratos: [Gastos](contracts/expenses.md), [OpenAPI](contracts/expenses-openapi.json), [matriz de aceptación](contracts/expense-acceptance.md). Operación de desarrollo: [runbook](development/expenses-runbook.md).

Migraciones 0030 expenses, 0031 referencias/comprobantes y 0032 índice Core documental. ISV 15% legacy decimal/half-up, HNL/USD separados, proveedor libre o snapshot autorizado, permisos por registro, versiones e intención auditada; doce replays, competencia, rollback y revocación. Papelera de gasto/documento conserva datos y bytes. Vínculos Process/Quote/OC/Invoice documentales/versionados mediante puertos propietarios; no modifican costos, impuestos, pagos ni estados de fuente. Comprobantes privados con cuarentena y scanner Core autorizado; no OCR ni registro automático.

La aceptación final corrigió permiso view adicional en writes/replays, bloqueos coordinados de UI, recuperación por etapa/intención y descartes de respuestas tardías. Core usa orden actor→propietario→documento y revalida binding; carrera de dos actores aprobada. Metadata se pagina por scope en SQL y el binding usa ID canónico. La consulta de archivos es opcional por permisos, sin bloquear registro manual.

CI usa Docker Hub oficial con el mismo digest OCI fijado de MySQL 5.7.44. El registro ECR anterior devolvió `Data limit exceeded` antes de pruebas; se verificó igualdad de hash y se recuperó la ejecución sin cambiar versión ni artefacto.

## Próximo paso exacto — Reportes

Leer [spec de Reportes](modules/reports.md), [plan](development/plan.md), Core/datos y contratos propietarios de Invoice/OC/Expense. Definir contrato, permisos/migración si corresponde y unidades verificables antes de código.

Primero: tres reportes canónicos Facturas/Órdenes/Gastos con filtros/fechas (`invoice_date`, `order_date`, `expense_date`), filas y totales autorizados por permisos del reporte **y** fuente, agregación SQL y moneda explícita. Inicio debe consumir la misma proyección para coincidir en rango/estado/scopes. Excluir anuladas y papelera conforme a propietario; no inventar ni duplicar gastos de inversión por referencias documentales.

Después: exportación Excel autenticada/expirable con reauth y mismos filtros/permisos, sin pérdida silenciosa de precisión ni fórmulas externas; aceptación integral. Vistas de RFQ/IA y métricas con propietarios aún inexistentes se integran en sus fases, sin datos fabricados. No iniciar Colaboración antes de cerrar Reportes.

## Gates externos pendientes

S3/IAM y scanner externos; SMTP/IMAP real de Bluehost; Percona Bluehost remoto con TLS/túnel y mínimo privilegio; DNS/HTTPS/CORS/CSP/WAF; backup/restore, despliegue Lightsail/Bluehost y migración de datos reales; OAuth delegado/ChatGPT Work; carga/memoria conjunta de crawler/API/workers. Configuración/rango/fiscalidad SAR productivos y casos de anulación de factura con pagos, documentos previos u OC vinculada requieren procedimiento coordinado antes de habilitarse.

Estos gates no se acreditan con adaptadores sintéticos. IA/OCR externa está en su fase; manual funciona sin ella. No OCR ni inferencia local en Lightsail ni proveedor pagado añadido como sustitución automática.

## Evidencia histórica

Detalle cronológico: [status](development/status.md), [plan](development/plan.md), [decisiones](development/decisions.md) y commits. El [checkpoint histórico previo](https://github.com/Ahcast/Bastion/blob/f3dd6121df2695cf89d9eb89c40dc28644f7a2c5/docs/ESTADO_DESARROLLO.md) conserva los cierres y bloqueos anteriores. Este documento compacto sustituye sus indicaciones de módulo activo antiguas.
