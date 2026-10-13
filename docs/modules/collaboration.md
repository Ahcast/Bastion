# Módulo Colaboración

Colaboración conserva la biblioteca privada por empresa/persona y conecta el trabajo diario entre módulos. Es un centro operativo ligero; no reemplaza Core.Files, Workflow, Google Drive/Slack ni la lógica propietaria de los módulos comerciales.

La especificación normativa, permisos, tablas, API y criterios de aceptación están en el [contrato de Colaboración](../contracts/collaboration.md), [OpenAPI](../contracts/collaboration-openapi.json), [matriz de aceptación](../contracts/collaboration-acceptance.md) y [runbook](../development/collaboration-runbook.md).

## Alcance implementado

### Unidad 1 — Biblioteca

- Carpetas de empresa/persona, equipo y jerarquía limitada.
- Metadata, búsqueda, versiones y papelera reversible.
- Upload/preview/download delegados a Core.Files; no duplica bytes ni claves S3.
- Cuarentena/análisis y políticas documentales Core conservadas.
- Scopes `all|self|team`, versión optimista, idempotencia, auditoría y reautenticación.

### Unidad 2 — Conversación

- Comentarios de texto plano, sin edición.
- Menciones y sugerencias sólo a usuarios con acceso vivo al recurso.
- Notificaciones mínimas sin copiar cuerpo/título.
- Actividad `comment_added|comment_removed`.
- Moderación con versión, idempotencia y reautenticación.
- Recursos reautorizados mediante políticas registradas; una mención nunca concede acceso.

### Unidad 3 — Mi trabajo

- Tareas propias con creador, asignado, título, nota, vencimiento `DATE` y estados `open|in_progress|done|cancelled`.
- Vistas `Mi trabajo`, `Hoy` y `Vencidas`.
- Asignación a usuarios activos/MFA y, si existe recurso, con acceso actual al dueño.
- Asignado controla avance/cierre; creador controla edición/reasignación.
- Tarea cerrada sólo se reactiva mediante acción explícita `reopen` con permiso, reautenticación y auditoría.
- No copia tareas ni decisiones de Workflow.

### Unidad 4 — Integraciones

- Vínculos documentales de una entrada de biblioteca a Proceso, Cotización, OC o Factura.
- Cada módulo propietario expone un puerto mínimo de lectura: propietario/equipo/etiqueta/versión y sus reglas terminales.
- Colaboración conserva snapshot de etiqueta/versión, muestra estado actual y marca `stale`; no reescribe historia ni consulta directamente tablas comerciales.
- Recurso borrado/anulado/inaccesible deja de exponerse.
- Bandeja de aprobaciones pendientes es una proyección de sólo lectura de Workflow; decisiones/revocaciones continúan exclusivamente en el módulo propietario.

## Pantallas

- `/collaboration`: biblioteca, archivos, conversación, menciones y actividad.
- `/collaboration/work`: tareas y vistas de trabajo.
- `/collaboration/integrations`: vínculos documentales y aprobaciones pendientes.

Toda UI está en español. El backend conserva DTOs/rutas técnicas en inglés.

## Límites de propiedad

- **Core.Files:** bytes, object key, hash, MIME, versiones físicas, cuarentena, análisis y URLs temporales.
- **Workflow:** solicitudes, aprobaciones, decisiones, revocación, versión/hash y reglas de aprobación.
- **Procesos/Cotizaciones/OC/Facturas:** autorización, estados terminales, versión y etiqueta del recurso.
- **Colaboración:** carpetas/entradas, conversación, tareas propias y relaciones documentales.

Ninguna operación de Colaboración modifica precios, impuestos, pagos, estados comerciales ni decisiones Workflow. IA/OCR no forma parte de estas cuatro unidades y no se ejecuta localmente en Lightsail.

## Seguridad

Permisos por acción y fila se validan en backend para cada lectura/escritura y replay. No se exponen claves S3 ni contenido activo inline. IDs ajenos/terminales/inexistentes responden sin enumeración. Las escrituras sensibles conservan MFA humano, reautenticación, versión optimista, idempotencia y auditoría.

## Certificación

El cierre exige una única revisión `main` con backend Go fmt/vet/race sobre MySQL 5.7 y builds, Angular build/unitarias, navegador real, documentación y política de rama única verdes. Las conexiones productivas externas se validan aparte y no quedan acreditadas por CI sintético.


Cierre U1–U4 y aceptación integral: Código `70163c9cb074a6285188cc812bdf9a408dfe8659`: [Core checks 38338667906](https://github.com/Ahcast/Bastion/actions/runs/38338667906), [documentación 38338667902](https://github.com/Ahcast/Bastion/actions/runs/38338667902) y [rama única 38338667908](https://github.com/Ahcast/Bastion/actions/runs/38338667908): success. Go fmt/vet/race sobre MySQL5.7 y builds server/worker/admin; Angular build/169 pruebas; navegador 21/21. Todas las unidades U1–U4 y aceptación integral cerradas para el alcance de desarrollo sintético.

Las capacidades externas de S3/scanner/Percona/SMTP/DNS y despliegue siguen fuera de este cierre. Próxima fase API/MCP IA; sin nuevo menú.
