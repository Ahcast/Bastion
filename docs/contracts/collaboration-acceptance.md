# Matriz de aceptación — Colaboración

Contrato de cierre para Colaboración 0.1.0. La fuente normativa es [collaboration.md](collaboration.md) y su API es [collaboration-openapi.json](collaboration-openapi.json).

## Regla de certificación

El módulo sólo se considera cerrado cuando la misma revisión de `main` aprueba: formato/vet, `go test -race -count=1 ./...` contra MySQL 5.7.44, builds Go, build y unitarias Angular, navegador contra API/Angular/MySQL reales de CI, documentación y política de rama única. Las conexiones productivas externas no se acreditan con adaptadores sintéticos.

## Unidad 1 — Biblioteca

| Caso | Aceptación |
| --- | --- |
| Carpetas | Empresa/persona, padre, equipo, profundidad, versión y papelera respetan scope y no cruzan propietarios. |
| Archivos | La entrada referencia Core.Files; no duplica bytes, claves S3 ni URLs persistentes. |
| Carga | Sólo upload finalizado y `quarantined|available`; pending/rejected/generated/ajeno/duplicado se rechaza. |
| Acceso | Cada metadata/preview/download/version/papelera/restore revalida Colaboración y Core.Files. |
| Concurrencia | Versión obsoleta y dos actores no pisan metadata ni ubicación. |
| Idempotencia | Replay exacto no duplica; replay cambiado devuelve conflicto. |
| Reautenticación | Vínculo, papelera/restore y acciones sensibles exigen sesión humana recientemente confirmada. |

## Unidad 2 — Conversación

| Caso | Aceptación |
| --- | --- |
| Comentarios | Texto plano hasta 4000 caracteres, sin edición y sin interpretación HTML. |
| Menciones | Máximo 25 IDs únicos; destinatario activo y con acceso vigente al mismo recurso. |
| Recursos | U2/U4 usan políticas registradas; IDs ajenos/terminales/inexistentes no se enumeran. |
| Notificaciones | No almacenan cuerpo/extracto; se omiten cuando el destinatario perdió acceso. |
| Actividad | Sólo hechos mínimos `comment_added|comment_removed`, sin copiar detalles de auditoría. |
| Moderación | Autor o moderador; borrado lógico, versión, idempotencia y reautenticación. |
| Navegador | Upload privado, cuarentena, conversación, mención, notificación, texto plano y papelera reversible. |

## Unidad 3 — Trabajo

| Caso | Aceptación |
| --- | --- |
| Creación | Título, nota, asignado y fecha `DATE`; recurso opcional. |
| Asignado | Usuario activo/MFA; si hay recurso, debe conservar acceso vivo al módulo propietario. |
| Vistas | `Mi trabajo`, `Hoy` y `Vencidas` se derivan de tareas propias; no copian Workflow. |
| Estado | `open → in_progress|done|cancelled`; asignado cambia estado. |
| Edición | Creador con `colaboracion.task`; versión optimista; tarea cerrada no se edita. |
| Reapertura | Acción explícita del creador, permiso vigente, reautenticación y auditoría. |
| Idempotencia | Create/update/status/reopen no duplican efectos; payload distinto con misma clave entra en conflicto. |
| Aislamiento | Tarea sin recurso sólo visible a creador/asignado; tarea vinculada desaparece si el recurso deja de ser legible. |

## Unidad 4 — Integración

| Caso | Aceptación |
| --- | --- |
| Vínculos | Sólo Process/Quote/OC/Invoice registrados; máximo 20 únicos por entrada. |
| Propiedad | Colaboración no consulta tablas comerciales directamente; usa puertos de lectura propietarios. |
| Snapshot | Guarda etiqueta/versión observada, muestra versión actual y marca `stale` sin reescribir historia. |
| Terminal/revocación | Recurso borrado/anulado/inaccesible se omite y no se filtra su existencia. |
| Escritura | `colaboracion.link`, versión de entrada/recurso, idempotencia, transacción, auditoría y reautenticación. |
| Bytes | Un vínculo no copia ni mueve documentos del recurso propietario. |
| Aprobaciones | Bandeja sólo lectura; Workflow conserva solicitud/decisión/revocación/versionado. |
| Permisos Workflow | Sólo pendientes, no expiradas, con `view_permission` y `decision_permission` vigentes; solicitante/propietario no se presenta como decisor de su propia solicitud. |

## Regresión transversal

- Módulo deshabilitado conserva datos y retira transporte/políticas.
- No hay grants automáticos en migraciones; asignaciones de prueba viven sólo en fixtures.
- Scopes `all|self|team` se reevalúan en cada operación relevante.
- IDs BIGINT viajan como strings JSON; dinero no forma parte de Colaboración.
- MySQL objetivo es 5.7.44; no se usan primitivas SQL exclusivas de versiones posteriores.
- U1–U4 no añaden OCR/IA local ni servicios pagados implícitos.
- Ninguna acción de Colaboración cambia estados, montos, pagos o decisiones de Clientes/Procesos/Cotizaciones/OC/Facturas/Gastos/Workflow.

## Gates externos fuera de esta certificación

S3/IAM y scanner reales, Percona Bluehost remoto, DNS/HTTPS/CSP/WAF, correo real, backups/restores, despliegue Lightsail/Bluehost y OAuth/ChatGPT Work siguen requiriendo validación productiva separada.
