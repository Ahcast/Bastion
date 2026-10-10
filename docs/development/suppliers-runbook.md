# Ejecutar y validar Proveedores

Leer [Core](core-runbook.md), [contrato](../contracts/suppliers.md) y [OpenAPI](../contracts/suppliers-openapi.json). Go: backend/internal/modules/suppliers; Angular: frontend/src/app/modules/suppliers; migración global 0003_suppliers.sql. MySQL productivo permanece Bluehost. Las pruebas utilizan únicamente esquemas desechables locales/CI.

## Uso

Un administrador técnico crea roles con proveedores.view/create/update y scopes all/self/team, y los asigna explícitamente a empleados invitados con MFA. Bootstrap no recibe capacidades comerciales y el administrador inicial sigue protegido. Solo módulos habilitados y permisos efectivos muestran Proveedores en el menú.

Crear ficha; RTN opcional, país libre, contactos, categorías/marcas (una por línea), productos/MOQ, términos y evaluación humana. No confundir evaluación manual con una puntuación calculada. Una ficha duplicada canónica o por RTN da conflicto, incluso inactiva. Cambiar equipo requiere update all y reautenticación; alta en equipo ajeno create all. Inactivar/reactivar requiere reautenticación. No borrar ni fusionar.

Agregar notas o precios conserva las ediciones pendientes de la ficha y actualiza su versión. Una observación necesita URL de fuente y fecha/hora; se captura en hora de Honduras y se persiste UTC. La interfaz presenta timestamps en UTC−06:00 (America/Tegucigalpa), independientemente de la zona del navegador. Precio/MOQ viajan como cadenas exactas con punto decimal, sin coma/exponente/redondeo. Si no hay precio publicado dejar precio y moneda vacíos. Impuestos/envío sin confirmar permanecen unknown, no se presume inclusión. Registrar fuente no descarga páginas ni envía mensajes ni crea una cotización oficial. Las observaciones no pueden editarse/borrarse desde la aplicación.

Ante 409 conservar edición y reabrir ficha para comparar antes de una nueva intención. No reintentar escrituras a ciegas. Historial, notas, precios y documentos están paginados. Archivos subidos desde Configuración requieren vínculo con ambos permisos, versión y reautenticación; cuarentena sigue bloqueando descarga. No introducir claves S3 ni URLs firmadas como sustituto del archivo Core.

## Pruebas

Desde backend, configurar BASTION_TEST_MYSQL_DSN hacia bastion_test local desechable MySQL 5.7: gofmt (sin cambios pendientes), go vet ./..., go test -race -count=1 ./... y make build. No apuntar a la BD productiva. Fixtures eliminan/recrean únicamente el esquema bastion_test y comprueban versión 5.7. CI ejecuta el mismo contrato contra contenedor MySQL 5.7.44 fijado por digest.

Desde frontend: npm ci; npm run build; npm test -- --watch=false. Navegador: crear bastion_browser_test mediante go run ./tests/browserseed (solo ese nombre permitido), iniciar API/Angular de desarrollo y npm run test:e2e. Las cuentas/roles son datos sintéticos exclusivos de ese esquema; no copiar seed a producción.

Casos: duplicados canónicos/RTN entre ocho actores; doce altas/observaciones con una clave; rollback de agregado/auditoría/hecho/intención; ocho modificaciones con una sola versión ganadora; scopes/IDOR/revocación, cambio de equipo/inactividad/reauth; decimales exactos, fuente/fecha y evidencias sin precio; snapshots/legado por texto, filtros literales y paginación; archivos/cuarentena; SDK MCP y REST con IDs mayores a 2^53. UI conserva edición/clave ante fallo, texto pendiente al añadir notas/precios y el ID confirmado si falla recarga tras POST. Navegador verifica contacto/producto/MOQ, precio exacto, persistencia tras recarga, duplicado e inactivación; regresa Core/Clientes.

## Desactivación y límites

BASTION_SUPPLIERS_ENABLED=false y reinicio bloquean rutas, tools, menú y política de archivos; conservar datos y migraciones. Rehabilitar exige la misma versión de esquema. Se ofrecen solo tools de lectura; OAuth Work y escrituras de agentes siguen pendientes.

RFQ/aprobación/envío, candidatos/investigación automática, cotizaciones recibidas oficiales, selección en OC/Gastos y métricas calculadas se aceptarán en las fases propietarias. No están activados por este catálogo. Estado/evidencia local y CI en [status](status.md); S3/SMTP/Bluehost reales no se acreditan con estos tests.
