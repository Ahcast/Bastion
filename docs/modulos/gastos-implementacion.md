# Gastos: registro general y preparación de activación

Fecha: 2026-10-25. El usuario autoriza completar el despliegue y activar este corte únicamente para el Administrador del sistema. La instalación y activación de las correcciones siguientes quedan pendientes del artifact aprobado por CI.

## Alcance

Registro general independiente con listado, búsqueda, fechas inclusivas, paginación, resumen de los resultados filtrados, alta, consulta, edición y borrado lógico. API, interfaz y cálculo previo usan el mismo cálculo decimal del servidor. Identidad, CSRF, permisos, auditoría y activación son servicios del Core.

No se enlazan gastos con Órdenes/Facturas ni se importan datos Legacy. Reportes conectados, exportación y MCP permanecen fuera de este corte.

## Cotejo con CastilloIT

Se revisó estáticamente la copia proporcionada en Downloads: `index.html` / `main-4ZA7RVJ2.js` enlazan `chunk-LIUI2WBL.js` (ExpensesComponent); backend `back/app/Http/Controllers/GastosController.php` y rutas `back/routes/api.php`. No se leyeron el manifiesto privado de gastos ni credenciales Legacy.

- Formulario activo: fecha, factura y proveedor requeridos; HNL es la única moneda ofrecida; una de las bases debe ser positiva. El backend limita factura a 50 caracteres y proveedor a 255.
- El backend redondea primero las bases a centavos, calcula ISV 15% sobre la base gravada normalizada y redondea el impuesto; suma ambas bases y el impuesto. Para importes no negativos, mitad hacia arriba. Bastion representa decimales exactamente y no replica errores binarios de float. Este cotejo acredita la secuencia del código; no cierra la revisión contable de negocio.
- La consulta general no filtra por creador; busca factura/proveedor ignorando mayúsculas y acentos, aplica fechas inclusivas y resume todos los resultados antes de paginar. Página inicial 20, máximo 100. Bastion añade ID descendente para estabilizar fechas iguales y rechaza intervalos invertidos.
- El borrado original asigna `deleted_at`, conserva los datos y los excluye de listado/edición. Bastion incorpora ese comportamiento mediante migración 2. No hay restauración ni purga confirmadas para este registro; no se añaden botones de ese tipo.
- El resumen conserva cantidad, bases gravada/exenta, impuesto y total del conjunto filtrado, independiente de la página. El editor muestra la vista previa calculada en Rust.

## Contrato técnico

- Permisos explícitos del Core: `gastos/registros/ver`, `editar` y `eliminar`; alta/edición comparten editar y el borrado tiene permiso separado. Sólo el Administrador del sistema queda autorizado por la solicitud de activación; no se amplían otros roles.
- Alta idempotente por actor y clave; reusar una clave con otro contenido devuelve 409. Edición con ETag/If-Match; versión obsoleta devuelve 412 y conserva el borrador. El formulario envía exclusivamente los seis campos de entrada.
- Límites técnicos de Bastion: 12 dígitos enteros y 4 decimales de entrada; persistencia DECIMAL(14,2), rechazo de negativos y desbordamiento; factura 50, proveedor 255, búsqueda 200; página máxima 10000; cuerpo 16 KiB, 120 solicitudes por actor/minuto y timeout 30 s. Los límites operativos no se presentan como reglas observadas del Legacy.
- Tablas `gastos_records`, `gastos_create_receipts`, `gastos_actor_limits`; ledger `_gastos_migrations`. Migración 1 crea el registro; migración 2 agrega `deleted_at` e índice para registros activos. El arranque verifica todas las versiones/checksums y la allowlist antes de declarar disponibilidad.
- `GET/POST /api/v1/gastos`, `GET/PUT/DELETE /api/v1/gastos/{id}`. `POST /api/v1/gastos/calculo` es transporte técnico para el cálculo previo, sin guardar registros y con editar/CSRF. Ver [OpenAPI](gastos.openapi.yaml).

## Verificación y publicación

Las pruebas específicas abarcan normalización/mitades/rangos, migraciones exactas y bloqueo sin disponibilidad, concurrencia idempotente, conflictos de edición, filtros/summary, permisos separados, auditoría, borrado lógico y cuotas. HTTP verifica sesión, CSRF/origen, seis campos, cálculo sin persistencia, ETag y tamaño de cuerpo. CI usa bases MySQL 5.7 desechables dedicadas `bastion_gastos_test` y `bastion_gastos_http_test`.

La suite web local completa pasó: 113 pruebas, tipos y build. Incluye regresión del reenvío de campos calculados al editar, borrador conservado ante 412, permisos de lectura/escritura/borrado, confirmación y paginación. El commit de código `a7757d3` pasó `Core checks` (run `40169034037`): compilación y pruebas del workspace, casos propios de cálculo, MySQL 5.7/HTTP y adjuntos aislados. Los artifacts de servidor y frontend se verificaron por SHA-256 y quedaron en staging privado de AWS/Bluehost. La instalación y la migración 2 se completaron el 2026-10-26 con el acceso existente expresamente autorizado por el usuario; sólo permanece pendiente la activación administrativa.

Al comenzar esta activación se verificó por SSH que AWS/Bluehost ejecutaban `ca383c7`; esquema inicial aplicado, readiness 200, `gastos` fuera de allowlist y desactivado. Ese estado inicial quedó superado por el despliegue verificado del 2026-10-26 que se detalla a continuación. Se preserva la arquitectura Bluehost/AWS/MySQL.

## Pendientes de alcance

Asociación a gastos de Órdenes/Facturas y reportes; migración y reconciliación del histórico Legacy; exportación/MCP; política futura de restauración/retención y equivalencia visual con una sesión Legacy real. No se declaran completados por la activación del registro general.

## Despliegue verificado del 2026-10-26

El 2026-10-26 se instalaron los artifacts de `a7757d3` (CI exitoso `40169034037`) en AWS y Bluehost. La versión previa inmediata era `d5e7ac5`, incluida en este release. Se verificaron respaldo privado de MySQL y frontend, conservación de las 61 tablas existentes y sus datos, y conservación de la activación de los demás módulos. Migraciones de Gastos 1 y 2 correctas; `deleted_at` incorporado, `gastos` permitido en el ambiente y disponible con `enabled=0`, pendiente de activación desde Ajustes por un Administrador. Readiness HTTPS 200, API anónima 401, 41 archivos frontend verificados en disco y 39 recursos públicos comprobados por HTTPS (se excluyen los dos .htaccess). Cero registros de gastos y cero permisos del módulo para otros roles; no se importaron datos Legacy ni se crearon registros de negocio sintéticos. Se preservaron config.js, .htaccess y assets anteriores para caché. Se utilizó el acceso MySQL existente autorizado por el usuario sólo para el proceso puntual de migración, sin crear cuentas, cambiar privilegios ni persistir variables de migrador.
