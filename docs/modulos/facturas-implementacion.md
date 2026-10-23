# Facturas: primer corte local

## Alcance y evidencia

Inicio solicitado el 2026-10-22. Revisión estática de `back/app/Http/Controllers/FacturasController.php` de la copia en Downloads: `validatedInvoice` (476–565) delega el cálculo comercial a `CotizacionesController::validatedQuote`, normaliza costo unitario, gastos y notas públicas; calcula inversión de productos como redondeo de la suma cantidad × costo, gastos adicionales, inversión total y utilidad (total con impuestos menos inversión). No se ejecutó Laravel ni se leyeron manifiestos, configuración privada o datos comerciales.

Este corte implementa borradores nuevos, listado/búsqueda/paginación, detalle, cliente manual o seleccionado del catálogo, ítems (descripción/precio/costo/cantidad/descuento/impuesto), descuentos, impuestos, gastos y notas privadas/públicas. Todos los cálculos usan enteros comprobados en Rust y la vista previa usa el mismo caso de uso que guarda. Se reutiliza exclusivamente el calculador puro de Cotizaciones; no requiere activar ese módulo ni accede a sus tablas.

Solo se guarda `Borrador`, sin número fiscal, pagos ni procedencia. No sustituye una factura emitida. Los términos `contado`/`credito` se conservan; crédito admite días y contado normaliza días a cero. Como contado requiere pago total, en este corte sin pagos únicamente puede guardarse contado con total cero; la UI explica la limitación antes de guardar. Los borradores de crédito pueden tener saldo pendiente. No se simulan ingresos ni estados Enviado/Pagado.

El `main-4ZA7RVJ2.js` activo carga listado `chunk-2VTWKLI6.js` y editor `chunk-C7YHIWLF.js`; la inspección inicial identificó alta en contado y cero días, costo, tracking, comentarios y notas públicas. La confirmación posterior del usuario del 2026-10-23 excluye tracking y comentarios de los ítems de factura y prevalece sobre esa inspección. Se conserva el default contado/cero días. Para preparar un borrador con saldo sin pagos en este corte se selecciona crédito explícitamente. Los importes de entrada y el total comercial normalizado no superan 999999999999.99 HNL; se rechaza un redondeo que exceda ese rango para mantener datos normalizados reeditables.

## Contrato técnico

OpenAPI: `facturas.openapi.yaml`, versión 0.2.0. Módulo `facturas`, alcance `documentos`; ver consulta listado/detalle y editar crea/actualiza/calcula. Documentos compartidos entre actores autorizados como en el controlador Legacy; no se presupone multitenancy. Core conserva sesión, CSRF, autorización y auditoría. El módulo nace desactivado; requiere migraciones propias correctas y `facturas` en `BASTION_ALLOWED_MODULES` antes de su activación administrativa.

- MySQL 5.7: tablas propias, ledger `_facturas_migrations`, JSON de datos y cálculo persistidos juntos, autores con claves foráneas. No se modifica el esquema de otros módulos.
- Creación con `Idempotency-Key` de 16–128 caracteres ASCII alfanuméricos/guion/guion bajo, vinculada al actor y contenido normalizado. Repetición devuelve el recibo original; otro contenido devuelve 409. Recibos sin vencimiento en este corte sin borrado.
- Detalle y escrituras devuelven ETag fuerte. Actualizar exige un único `If-Match` con versión positiva; versión obsoleta devuelve 412, sin sobrescribir. Estado y número nunca se aceptan del cliente.
- Bloqueo de registro de módulo compartido con su activación para serializar escrituras/idempotencia y desactivación; auditoría transaccional `invoice.draft.create`/`invoice.draft.update` sin notas ni datos del cliente.
- Límites técnicos nuevos (no atributos del Legacy): 200 ítems, 200 gastos, 1 MiB por petición, 120 solicitudes/minuto/actor, timeout HTTP 30 segundos; página 1..10000 y tamaño 1..100. Importes viajan como cadenas decimales sin signo/exponente, hasta 12 enteros y 4 decimales; normalización a centavos. Días 0..3650, fecha 1000..9999. Los límites y allowlist de campos se aplican en Rust.

## Diferencias y pendientes explícitos

Desde la confirmación del 2026-10-23, el listado usa un desplegable por fila con Ver, Editar, Marcar como enviado, Ingresar pago, Clonar, PDF, Nota de entrega, Emitir Recibo y Eliminar, en ese orden. Reutiliza el menú accesible de Cotizaciones (presentación sin reglas de negocio), con foco, flechas, Escape y cierre fuera del menú. Ver abre el borrador de solo lectura; Editar requiere editar. Las siete acciones aún pendientes muestran su motivo y no llaman endpoints futuros; Eliminar conserva su permiso independiente.

No hay numeración/SAR, emisión, envío/estados, pagos/recibos, PDF/nota de entrega, clonación, eliminación, conversiones, selección de proveedores ni MCP. Referencias de exoneración se conservan en dominio, sin añadir controles no verificados a la UI. Se preserva el cálculo cotejado de Cotizaciones, incluido su prorrateo sin redistribución de residuos; ver `cotizaciones-paridad.md`. El costo se asocia al mismo ítem al descartar filas vacías, evitando el desfase de índices observado en el merge Legacy. No se afirma validación fiscal/contable ni equivalencia visual sin sesión real y ejemplos anonimizados.

Ajustes muestra la etapa y los pendientes, sin inventar settings. Se mantienen Bluehost/AWS/MySQL. No se migran datos Legacy ni se instala este corte en servidores. El módulo sigue incompleto hasta cerrar las reglas de la ficha y sus ajustes.

## Validación

Validación local completada el 2026-10-22:

- `cargo check --workspace --locked --offline`, `cargo test --workspace --locked --offline` y rustfmt: correctos con Rust 1.99.1. Cuatro nuevas pruebas de dominio cubren cálculo/inversión, contado, asignación de costos después de filas vacías, importes inválidos/límites y campos no admitidos; el adaptador prueba cabeceras de versión fuertes/únicas.
- Las pruebas que requieren MySQL aislado y Typst no ejecutaron sus ramas operativas al faltar sus variables de entorno. Se preparan dos bases separadas de MySQL 5.7 en CI para Facturas (servicio y HTTP); **no se declara comprobada la persistencia contra MySQL con esta ejecución local**. CI no se ha ejecutado para este corte local.
- Frontend: tipos, 58 pruebas (cinco de Facturas) y build Vite correctos. Las pruebas nuevas comprueban permisos/navegación, fecha Honduras, decimales mostrados, conflicto 412 conservando datos, confirmación al salir, contado sin pago y clave estable ante respuesta de red ambigua.
- Revisión visual local con datos exclusivamente sintéticos: editor a 320, 390, 768 y 1440 px sin desbordamiento horizontal del documento, y sin errores de consola. No equivale a cotejo visual de una sesión real Legacy ni a prueba integral contra Rust/MySQL. Capturas de revisión conservadas en `.tools/facturas-editor-1440.png` y `.tools/facturas-editor-320.png` (archivos locales ignorados).

Actualización 2026-10-23: tipos, 59 pruebas frontend y build correctos tras incorporar el desplegable. La prueba adicional comprueba las nueve etiquetas y su orden exacto, permisos, acciones pendientes deshabilitadas con motivo, foco/flechas/Escape, apertura Ver/Editar y ausencia de mutaciones al usar el menú.

## Frontend alineado con Cotizaciones — 2026-10-23

Por solicitud del usuario se aplica `cotizaciones-frontend.md`: listado con regla oscura, búsqueda compacta, importes y referencia monoespaciados, cliente con referencia secundaria y contador limitado a la página; Generales compacto con referencia reducida; buscador único compartido para selección/manual; ítems en tabla adaptable con venta, costo, cantidad, descuento, impuesto, total y quitar. Por confirmación del usuario, los ítems no llevan tracking ni comentarios. Estos campos se retiran del editor y del contrato publicado; el servidor tolera su lectura en borradores anteriores, pero los omite al responder y guardar. Gastos conserva sus campos; notas, descuento y resumen quedan separados debajo. No se añade unidad de medida ni se calcula dinero en JavaScript. Los totales de línea no se muestran si el servidor filtró filas y su posición no puede comprobarse.

`id_client` es opcional y pertenece al catálogo de Bastion. Seleccionar otro cliente requiere Clientes activo y `clientes/catalogo/ver`; el contrato `ClientService::document_snapshot` resuelve nombre, RTN y correo dentro de la transacción. Se guarda una instantánea y editar otros campos conserva esos datos aunque Clientes se desactive. Volver a manual limpia la relación, RTN y correo. Los documentos anteriores siguen siendo manuales; no hay nueva migración ni se importan IDs Legacy. Las claves idempotentes se verifican antes de consultar el catálogo mutable, de modo que un reintento idéntico conserva su resultado original tras cambios en Clientes.

Se conserva el desplegable de nueve acciones solicitado, los permisos y los pendientes de emisión fiscal/pagos/documentos. Esta entrega se publica en GitHub; su publicación no instala ni activa el módulo en servidores.

Validación de la subida del 2026-10-23: paquete aislado basado en main actualizado, compilación y formato Rust correctos, pruebas de dominio locales correctas y 89 pruebas frontend/tipos/build correctos. Las pruebas con MySQL 5.7 se ejecutan en CI con bases aisladas; los tests locales sin esas variables no validan persistencia real. Se conserva la activación administrativa pendiente y no se instala este commit en servidores.
