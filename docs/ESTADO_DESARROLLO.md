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
| Reportes | Tres unidades canónicas cerradas/certificadas: consultas Facturas/Órdenes/Gastos, Inicio, Excel privado y aceptación integral |
| Colaboración | U1–U4 implementadas; aceptación integral en revisión. No certificada aún |
| Workflows configurables e IA | Fases posteriores según dependencias; no confundir Workflow de aprobación ya integrado con configuración futura |
| Producción | No desplegada ni migrada; integraciones externas no acreditadas por CI |

## Cierre certificado de Gastos

Código `f3dd6121df2695cf89d9eb89c40dc28644f7a2c5`:

- [Core checks 38300556293](https://github.com/Ahcast/Bastion/actions/runs/38300556293): Go fmt/vet, `go test -race -count=1 ./...` sobre MySQL 5.7, builds server/worker/admin, Angular build/**143 pruebas** y navegador **18/18** contra API/Angular/BD reales de CI: success.
- [Documentación 38300556205](https://github.com/Ahcast/Bastion/actions/runs/38300556205) y [rama única 38300556380](https://github.com/Ahcast/Bastion/actions/runs/38300556380): success.
- Contratos: [Gastos](contracts/expenses.md), [OpenAPI](contracts/expenses-openapi.json), [matriz de aceptación](contracts/expense-acceptance.md). Operación de desarrollo: [runbook](development/expenses-runbook.md).

Migraciones 0030 expenses, 0031 referencias/comprobantes y 0032 índice Core documental. ISV 15% legacy decimal/half-up, HNL/USD separados, proveedor libre o snapshot autorizado, permisos por registro, versiones e intención auditada; doce replays, competencia, rollback y revocación. Papelera de gasto/documento conserva datos y bytes. Vínculos Process/Quote/OC/Invoice documentales/versionados mediante puertos propietarios; no modifican costos, impuestos, pagos ni estados de fuente. Comprobantes privados con cuarentena y scanner Core autorizado; no OCR ni registro automático.

La aceptación final corrigió permiso view adicional en writes/replays, bloqueos coordinados de UI, recuperación por etapa/intención y descartes de respuestas tardías. Core usa orden actor→propietario→documento y revalida binding; carrera de dos actores aprobada. Metadata se pagina por scope en SQL y el binding usa ID canónico. La consulta de archivos es opcional por permisos, sin bloquear registro manual.

CI usa Docker Hub oficial con el mismo digest OCI fijado de MySQL 5.7.44. El registro ECR anterior devolvió `Data limit exceeded` antes de pruebas; se verificó igualdad de hash y se recuperó la ejecución sin cambiar versión ni artefacto.

## Reportes — alcance canónico cerrado

Tres unidades: consultas/proyección de Inicio, exportación Excel y aceptación integral. Contratos [Reportes](contracts/reports.md), [Excel](contracts/reports-export.md), [OpenAPI](contracts/reports-openapi.json), [matriz](contracts/reports-acceptance.md) y [runbook](development/reports-runbook.md).

- Unidad 1: `6e615338d0e5310c33dda05682f4bdfa2776186f`, [Core checks 38309240350](https://github.com/Ahcast/Bastion/actions/runs/38309240350), success: Go/race/MySQL5.7/builds, Angular 148 pruebas, navegador 19/19. Las correcciones intermedias de etiquetas/ACK/MFA sintético quedan documentadas en status.
- Excel y aceptación: código `84fcc123e1ea2b10e11b0d0c78427a90f41ea553`, [Core checks 38310992510](https://github.com/Ahcast/Bastion/actions/runs/38310992510), success: Go fmt/vet/race/MySQL5.7/builds, Angular build/152 pruebas, navegador 19/19 con descarga real XLSX. [Documentación 38310992497](https://github.com/Ahcast/Bastion/actions/runs/38310992497) y [rama única 38310992521](https://github.com/Ahcast/Bastion/actions/runs/38310992521), success. Lector openpyxl independiente local verificó libro sintético, precisión, fórmulas/cache y filtros.

Migración 0033 registra permisos sin grants. Reportes intersecta scopes propios/fuente; Excel agrega scope export/reauth/humano/CSRF. Proyecciones SQL sobre fechas DATE canónicas; anuladas/papelera excluidas; todos los montos strings/DECIMAL, monedas separadas, filas/totales en snapshot REPEATABLE READ. Inicio usa misma función sin estado específico; conciliar mismo período/moneda y reporte sin filtro de estado. Referencias documentales no vuelven a sumar gastos ni modifican utilidad.

Excel privado por stream, sin URL reutilizable/archivo persistido: texto exacto, fórmulas auxiliares de centavos con cota SUM ABS SQL, dos hojas, filtros/UTC, auditoría antes de bytes y errores explícitos por 10001 filas/24MiB XML/concurrencia. Tests incluyen exportación de 128 filas independiente de página, límite 10001, montos grandes/utilidad negativa, permisos, reauth, CSRF, revocación y rollback de auditoría.

## Próximo paso exacto — aceptación integral de Colaboración

Código base `4211f5e` y corrección UI `43418a7`: unidades U1–U4 ya existen. Última CI base 38331234661 aprobó backend Go/race/MySQL5.7/builds y Angular; browser 18/20, con fallos en selector de asignado y campo Para de correo. `43418a7` agrega etiqueta explícita y destinatario reactivo; build/163 pruebas Angular locales aprobados, CI 38332910384 aprobada completa: Go/race/MySQL5.7/builds, Angular/163 pruebas y navegador 20/20; docs 38332910388 y rama única 38332910353 aprobadas.

Revisión de aceptación agrega reautorización `colaboracion.view` para tareas personales y recuperación de replay tras completar/reabrir; restricciones de estado/version se ejecutan dentro del efecto idempotente. Cierra cursor de candidatos antes de autorizar asignados, probado con pool de una conexión. Edición/reasignación/paginación de tareas y recuperación UI U3/U4 implementadas; build/169 pruebas Angular locales aprobados. Integración ampliada con rollback, doce creates concurrentes, aislamiento y carrera real creator/update vs assignee/status; nombre del asignado no se bloquea mediante JOIN. Nuevo browser cubre edición. Revisión nueva requiere MySQL5.7/Go race/regresión de 20 escenarios; no cerrar módulo ni empezar la siguiente fase hasta regresión completa verde. Contrato/OpenAPI/matriz/runbook son la entrada actual.

Ampliaciones analíticas de RFQ/IA, competitividad de proveedores y estados de procesos no modelados se integran por contratos propietarios en sus fases, sin datos fabricados ni cálculos de modelos. No se acredita aún el alcance completo de todas las categorías analíticas futuras del maestro. Reportes canónicos funciona sin IA.

## Gates externos pendientes

S3/IAM y scanner externos; SMTP/IMAP real de Bluehost; Percona Bluehost remoto con TLS/túnel y mínimo privilegio; DNS/HTTPS/CORS/CSP/WAF; backup/restore, despliegue Lightsail/Bluehost y migración de datos reales; OAuth delegado/ChatGPT Work; carga/memoria conjunta de crawler/API/workers. Configuración/rango/fiscalidad SAR productivos y casos de anulación de factura con pagos, documentos previos u OC vinculada requieren procedimiento coordinado antes de habilitarse.

Estos gates no se acreditan con adaptadores sintéticos. IA/OCR externa está en su fase; manual funciona sin ella. No OCR ni inferencia local en Lightsail ni proveedor pagado añadido como sustitución automática.

## Evidencia histórica

Detalle cronológico: [status](development/status.md), [plan](development/plan.md), [decisiones](development/decisions.md) y commits. El [checkpoint histórico previo](https://github.com/Ahcast/Bastion/blob/f3dd6121df2695cf89d9eb89c40dc28644f7a2c5/docs/ESTADO_DESARROLLO.md) conserva los cierres y bloqueos anteriores. Este documento compacto sustituye sus indicaciones de módulo activo antiguas.
