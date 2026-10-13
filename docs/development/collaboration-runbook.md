# Runbook de desarrollo — Colaboración

## Objetivo

Verificar Colaboración 0.1.0 sin tocar producción. El módulo cubre biblioteca privada, conversación/menciones, tareas propias y vínculos/aprobaciones de sólo lectura. Core.Files, Workflow y módulos comerciales conservan su propiedad.

## Entorno

- Go y Node según el repositorio.
- MySQL 5.7.44 aislado; nunca BD productiva.
- `BASTION_COLLABORATION_ENABLED=true`.
- Para U4, habilitar únicamente los módulos propietarios que se quieran probar y Workflow si se prueba la bandeja.
- Adaptador de objetos sintético/local en pruebas; no usar S3 productivo como requisito de CI.

## Verificación backend

Desde `backend`:

```bash
gofmt -w ./internal/modules/collaboration ./internal/modules/processes ./internal/modules/quotes ./internal/modules/purchaseorders ./internal/modules/invoices ./internal/modules/workflow ./internal/composition ./tests/integration ./tests/browserapp
go vet ./...
go test -race -count=1 ./...
make build
```

La integración debe ejecutar migraciones hasta 0037 y probar contra MySQL 5.7 real. Sin DSN de integración, un test omitido no acredita persistencia.

## Verificación frontend

Desde `frontend`:

```bash
npm ci
npm run build
npm test -- --watch=false
```

Comprobar además:

- `/collaboration`: biblioteca y conversación.
- `/collaboration/work`: Mi trabajo/Hoy/Vencidas, creación y transiciones.
- `/collaboration/integrations`: vínculos documentales y bandeja Workflow de sólo lectura.
- No mostrar acciones sin los permisos correspondientes.

## Navegador

El workflow Core levanta MySQL 5.7, API de prueba y Angular y ejecuta Playwright sin mocks HTTP. El fixture de browser sólo puede actuar sobre `bastion_browser_test`; nunca debe introducir grants o identidades en producción.

El flujo U1/U2 debe cubrir carga privada, cuarentena, texto plano, mención/notificación, revocación y papelera reversible. U3/U4 también tienen recorrido de navegador: crear/editar/completar/reabrir tarea y agregar/quitar vínculo, además de integración MySQL y unitarias Angular. El recorrido debe conservar MFA/TOTP/reauth reales y no añadir bypass de autenticación.

## Pruebas de seguridad mínimas

1. Revocar `colaboracion.view` o permiso propietario y repetir lectura/replay: debe desaparecer el recurso.
2. Tarea vinculada: el asignado debe tener acceso vivo al recurso al crear y al leer.
3. Tarea cerrada: `/status` no la reactiva; sólo `/reopen` con creador, permiso y reauth.
4. Vínculo U4: recurso terminal/borrado/anulado deja de exponerse; snapshot histórico no se reescribe.
5. Aprobaciones: lectura no cambia `approvals.status`; las decisiones siguen en Workflow.
6. Replays: misma clave+mismo payload conserva un solo efecto; mismo key+payload distinto devuelve 409.
7. Versiones: entrada/tarea/recurso obsoletos devuelven 409.
8. Comentarios y menciones no conceden acceso.

## Diagnóstico

- `403`: permiso faltante o reautenticación requerida; no convertir automáticamente en grant.
- `404`: recurso ajeno, terminal o inexistente; no añadir mensajes que permitan enumeración.
- `409`: versión/replay/transición incompatible; recargar snapshot antes de reintentar.
- `422`: DTO o transición inválidos.
- `503`: dependencia registrada pero no disponible; no simular éxito.

## Cierre

Sólo cerrar el módulo cuando el mismo HEAD tenga verdes:

1. Core checks: backend fmt/vet/race/MySQL5.7/builds, Angular build/unit, browser.
2. Documentation checks.
3. Single branch policy.

Después registrar SHA/run IDs en `docs/ESTADO_DESARROLLO.md`. La certificación no equivale a despliegue ni valida S3/scanner/SMTP/Percona/DNS productivos.


## Evidencia vigente de cierre

Código `70163c9cb074a6285188cc812bdf9a408dfe8659`: [Core checks 38338667906](https://github.com/Ahcast/Bastion/actions/runs/38338667906), [documentación 38338667902](https://github.com/Ahcast/Bastion/actions/runs/38338667902) y [rama única 38338667908](https://github.com/Ahcast/Bastion/actions/runs/38338667908): success. Go fmt/vet/race sobre MySQL5.7 y builds server/worker/admin; Angular build/169 pruebas; navegador 21/21. Todas las unidades U1–U4 y aceptación integral cerradas para el alcance de desarrollo sintético.

La aceptación transversal usa servicios propietarios reales y dos humanos en navegador. Casos y resultados exactos en la matriz de aceptación y checkpoint.
