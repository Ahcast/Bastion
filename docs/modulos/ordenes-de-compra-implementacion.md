# Órdenes de compra: primer corte

## Alcance y evidencia

Solicitud de continuación del 2026-10-23. El repositorio tenía la ficha de paridad y la fila de permisos, sin implementación del módulo. Se revisó estáticamente `OrdenCompraController.php` de la copia local en Downloads (`calculateOrderTotals`, `normalizeExpenses`, `resolveOrderStatusId`, `create`, `update`, `markDelivered`) y `castillo-order-inline-edit.js`. No se ejecutó Laravel, consultó datos productivos ni importó catálogos reales.

El usuario confirmó agregar el catálogo de destinos bajo **Ajustes > Órdenes de compra**. Es una ampliación autorizada del nuevo producto: el Legacy solo confirma su consulta/selección. El catálogo pertenece al dominio de Órdenes en este corte; no depende de activar Proveedores. El futuro módulo de Proveedores consumirá su contrato si requiere destinos, sin escribir sus tablas. No se añaden otros defaults o políticas configurables.

Implementado: listado paginado, búsqueda por orden/factura/cliente, filtros por estado e importes, detalle de solo lectura, alta/edición de cabecera, cliente manual/RTN, productos, cantidades enteras, costo/venta, ISV, tracking, comentarios y gastos. El estado se deriva en Rust: factura no vacía → Entregado; todos los productos Comprado y lista no vacía → Listo; demás casos → Pendiente. Los estados de producto de este corte son Pendiente/Comprado, comprobados en el controlador; no se inventan identificadores numéricos de los catálogos antiguos.

La UI permite agregar/renombrar destinos según permiso. No permite borrarlos; sus identificadores permanecen estables. Un destino vacío, inexistente o inválido no se puede guardar en una orden. Si no hay destinos, el editor indica cómo agregarlos y bloquea el guardado.

## Cálculo y paridad

Los nombres Legacy están invertidos respecto de su lectura habitual: **`sale_price` es costo** e integra inversión, mientras **`purchase_price` es precio de venta** e integra subtotal/impuesto. La interfaz muestra las etiquetas comerciales comprobadas, manteniendo los nombres técnicos del contrato.

- Subtotal = suma de `purchase_price × cantidad`; subtotal exonerado = líneas sin ISV.
- ISV = 15% de la base gravada; total = subtotal sin redondear + impuesto sin redondear, redondeado al final.
- Inversión de productos = suma de `sale_price × cantidad`; otros gastos = gastos positivos redondeados individualmente a dos decimales. Los gastos cero se descartan como en Legacy.
- Inversión total = inversión de productos sin redondear + gastos normalizados, redondeada al final. La fórmula Legacy observada era total menos inversión total. El usuario la corrige el 2026-10-26: utilidad estimada = total de venta − ISV − inversión total (productos y gastos). Es una decisión autorizada que reemplaza la fórmula observada.
- El servidor usa enteros escalados, conserva hasta cuatro decimales de precio unitario y redondea los resultados a centavos con mitad hacia arriba. No delega totales al navegador. Las pruebas cubren la agregación antes de redondear y la estabilidad al reeditar. No se afirma equivalencia de los errores binarios del float PHP ni validación contable con ejemplos reales.

## Contrato e integración

OpenAPI: `ordenes-de-compra.openapi.yaml`, escrito antes de implementar handlers. Identidad estable `ordenes_compra` (ya presente en el catálogo de permisos), API `/api/v1/ordenes-compra`. Permisos `documentos/ver` para listado/detalle, `documentos/editar` para calcular/crear/actualizar. Destinos usa `destinos/ver`/`destinos/editar`; leer el selector también admite `documentos/ver`. El Administrador conserva acceso completo. Registros compartidos entre actores autorizados, como en el controlador revisado; no se presume un tenant.

Core conserva sesión, CSRF, origen, autorización y auditoría; no hay identidad paralela ni herramienta MCP. El módulo se incluye desactivado, exige migraciones propias exactas y `ordenes_compra` en `BASTION_ALLOWED_MODULES` antes de habilitarlo desde Ajustes > Core > Módulos. Desactivar bloquea órdenes y destinos sin borrar datos.

MySQL 5.7: ledger `_ordenes_compra_migrations`, tablas `ordenes_orders`, `ordenes_destinations`, recibos y cuotas propios. Destino y autores tienen claves foráneas. Se persisten juntos cabecera, entrada normalizada y cálculo; no se modifican tablas de Clientes, Facturas o Cotizaciones. El catálogo de destinos es dato de dominio administrado desde Ajustes, no un setting JSON que sustituya el almacén común del Core.

Creación con `Idempotency-Key` ASCII de 16–128 caracteres, por actor/contenido normalizado, recibo original conservado sin vencimiento. Misma clave con otro contenido → 409. La UI congela el alta ante respuesta ambigua y reintenta con el mismo cuerpo y clave. Actualizaciones requieren un único ETag fuerte mediante `If-Match`; versión obsoleta → 412, manteniendo lo escrito en pantalla. La UI confirma antes de descartar cambios. El bloqueo del registro de módulo serializa escrituras y desactivación. Auditoría transaccional `order.create`, `order.update`, `destination.create` y `destination.update`, sin nombres/clientes/comentarios en los detalles.

Destinos: nombres de hasta 255 caracteres, espacios normalizados y unicidad de MySQL sin distinguir mayúsculas/acentos. Crear de nuevo un nombre equivalente devuelve el destino existente; renombrar exige su versión y rechaza duplicados. No se incluyen valores iniciales ficticios. Los demás settings siguen pendientes.

Límites técnicos nuevos, no atribuidos al Legacy: 200 productos/gastos por orden, cantidad entera 0..1000000, importes no negativos con hasta 12 enteros y 4 decimales, resultados limitados a 999999999999.99 HNL. Máximo 1000 destinos; página 1..10000, tamaño 1..100; búsqueda 200 caracteres, cuerpo 1 MiB, 120 solicitudes/minuto/actor, plazo HTTP 30 segundos. Campos inesperados se rechazan. Fechas de calendario 1000..9999; fecha inicial en Honduras y timestamps en UTC.

## Pendientes explícitos

Cliente manual se conserva en la orden como instantánea; todavía no crea, vincula ni fusiona registros en Clientes. El selector de cliente y filtro por ID quedan pendientes del contrato entre módulos. El precio/cantidad negativos que el Legacy no rechaza sistemáticamente se rechazan en este corte; no se replican valores corruptos.

No hay selector de proveedor, archivos privados, borrado, clonación, conversión a factura/cotización, origen de cotización, gastos generales ni MCP. Las acciones futuras se muestran deshabilitadas con motivo. Guardar una referencia de factura aplica el estado cotejado, pero no crea una factura ni ejecuta el comando separado Marcar entregada (que en Legacy completa fecha de entrega si falta); ese comando sigue pendiente. La edición permite fecha opcional, como el controlador update. La paridad sigue incompleta y no se migra información Legacy; la publicación técnica posterior se documenta al final.

## Verificación local

- Rust 1.99.1: `cargo check --workspace --locked --offline` correcto. `cargo test --workspace --locked --offline -- --test-threads=1` correcto (68 resultados de prueba, algunos con retorno temprano por dependencias opcionales). La primera ejecución paralela encontró una colisión preexistente de nombres temporales en `procesos::diagnostics`; no se cambió Procesos para resolverla.
- Tres pruebas nuevas de cálculo validan dirección costo/venta, ISV/exonerado, gastos/redondeo, utilidad negativa, estados, rangos y campos no admitidos. Prueba de API para cabeceras fuertes y únicas.
- Las ramas de integración MySQL **no se ejecutaron localmente**: no se configuraron `BASTION_ORDENES_TEST_URL` ni `BASTION_ORDENES_HTTP_TEST_URL`. CI prepara dos bases desechables separadas de MySQL 5.7 para servicio y HTTP; queda pendiente ejecutar CI. No se declara comprobada la persistencia SQL con esta ejecución local.
- Frontend: tipos, 64 pruebas y build correctos; cinco pruebas nuevas cubren activación, scopes, fecha Honduras, conflictos, cambios sin guardar, alta ambigua y catálogo vacío. Los tests con transportes sintéticos no sustituyen la prueba integral contra Rust/MySQL.
- Revisión visual con datos sintéticos del listado, editor y catálogo de Ajustes; editor a 320 px sin desbordamiento horizontal. Sin errores de consola durante la revisión de listado/editor. No equivale a cotejo con sesión activa Legacy.

Se conserva el despliegue frontend Bluehost, backend/API/MCP AWS y MySQL Bluehost.

## Publicación autorizada (2026-10-23)

El usuario solicita subir a GitHub y después a AWS. Se prepara un release de Órdenes sobre main vigente, conservando los cambios ya publicados y excluyendo trabajos locales de otros módulos. Publicación y despliegue quedan sujetos a CI exitoso, artifacts de ese commit verificados, respaldo privado, migración aditiva y salud HTTPS. El frontend sigue en Bluehost; el backend en AWS. La instalación no activa el módulo automáticamente ni crea destinos de ejemplo.

Preparación del release sobre main actualizado: 68 pruebas frontend, comprobación de tipos y build correctos. El conteo incluye pruebas ya publicadas del Dashboard y no incorpora el trabajo local de Facturas. La integración MySQL se verificará en CI antes de instalar.

## Publicación verificada — 2026-10-23

Código publicado en [59a8057564cc6a72b3f5213f4283b488f0343f09](https://github.com/Ahcast/Bastion/commit/59a8057564cc6a72b3f5213f4283b488f0343f09), después del [CI exitoso](https://github.com/Ahcast/Bastion/actions/runs/39998520884): formato, compilación, 66 pruebas Rust (incluida integración real con dos bases aisladas de MySQL 5.7 para Órdenes), 68 pruebas frontend, tipos y build.

Se verificaron los SHA-256 de los ZIP, paquetes y archivos antes de instalar. Backend actualizado en AWS y 34 archivos de frontend publicados en Bluehost, con respaldo privado, conservación de cabeceras/assets anteriores y cambio atómico del HTML. Migración propia versión 1 aplicada; sin importar datos Legacy ni crear órdenes o destinos de prueba.

La primera instalación se revirtió automáticamente al detectar diferencias en CHECKSUM TABLE. La inspección comprobó que esos checksums no eran repetibles en las tablas JSON y que los INSERT de negocio seguían idénticos al respaldo. Las diferencias reales eran el ledger nuevo y la limpieza de desafíos OTP vencidos al restaurar el servicio. El reintento conservó las tablas aditivas vacías verificadas y comparó hashes de datos lógicos y esquemas de todas las tablas existentes antes y después de migrate; la comparación fue correcta.

Comprobación HTTPS: frontend BUILD del mismo commit y hashes de 21 recursos públicos correctos; API live/ready 200, Órdenes y Destinos anónimos 401. Se conservaron datos de negocio, activación de los módulos existentes y configuración de documentos. Órdenes queda incluido y permitido, **desactivado hasta su activación administrativa en Ajustes > Core > Módulos**; el catálogo de destinos comienza vacío. La validación operativa con usuarios, las funciones pendientes y la restauración integral de respaldos siguen abiertas.

## Integración de conversiones — 2026-10-23

Creación desde Cotizaciones implementada mediante document-flows, completando número y destino y conservando origen. Cantidades, descuentos, impuestos y redondeo incompatibles se rechazan explícitamente; la conversión a Facturas desde Órdenes sigue pendiente. Ver cotizaciones-conversiones.md.
