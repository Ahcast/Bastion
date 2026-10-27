# Empresa y numeración SAR

El usuario solicita completar el ajuste de numeración SAR de Facturas el 2026-10-23. Se implementa como configuración global del Core, exclusiva del Administrador del sistema, conforme a `02-core-y-ajustes.md`. Se presenta en **Ajustes > Facturas > Numeración SAR** y **Ajustes > Core > Empresa y SAR**, sin duplicar configuración ni depender de que Facturas esté activo para configurar desde el Core.

## Paridad y alcance

Referencia estática: `CompanySettingsController.php` de la copia de CastilloIT, métodos `update`, `sarStatus` y `lastEightNumber`. Campos observados: `limit_range` (rango autorizado, hasta 255 caracteres), `limit_date` (fecha opcional), `current_invoice` (factura actual, hasta 80 caracteres) y `sar_text` (hasta 3000 caracteres). Se preservan los cuatro campos; no se añade un campo CAI independiente ni un contador nuevo. Los datos generales de empresa continúan en `company.profile`. Logo y emisión fiscal siguen fuera de este corte.

El backend recorta espacios exteriores y valida tipos, longitudes, fecha real y campos permitidos. Fecha opcional, limitada al intervalo técnico 1000–9999. Los campos vacíos son válidos, como en Legacy. No se deduce una gramática fiscal nueva ni se comprueba una autorización ante SAR. Registrar estos datos no emite documentos, no asigna números a borradores y no habilita acciones pendientes.

Estado informativo calculado en Rust: rango toma el último grupo de dígitos y factura actual concatena los grupos; se interpretan los últimos ocho dígitos de cada valor, como Legacy. Facturas restantes = máximo de límite menos actual y cero. Sin números interpretables, el resultado es nulo, no cero. Días restantes = fecha límite menos fecha actual de Honduras; cero días aún no está vencido y valores negativos sí. Rango agotado cuando quedan cero facturas; no se inventan umbrales preventivos. La pantalla muestra el estado de los datos guardados y no calcula numeración en JavaScript. La corrección o sustitución de autorizaciones y la emisión real deben definirse antes de consumir correlativos.

## Persistencia y API

Se reutiliza `settings`, sin DDL ni migración: `core/global/company.sar`, esquema 1. Lectura y escritura exclusivas del Administrador activo; los permisos de roles personalizados no abren acceso al Core. La escritura vuelve a comprobar administración dentro de la transacción. Bloqueo del registro Core serializa tanto alta inicial como edición; el valor y el evento `settings.sar.update` se guardan juntos. El evento solo contiene versión de esquema, sin datos fiscales.

`GET /api/v1/settings/sar` devuelve `{settings, version, status}`. `PUT` en la misma ruta exige exactamente `{settings, version}`. `version` es una huella SHA-256 opaca del valor guardado (incluido estado sin configurar), no un campo fiscal. Se compara dentro del bloqueo; versión obsoleta responde 412 y no modifica datos ni auditoría. La ruta genérica de settings no acepta `company.sar`, evitando eludir este control. Sesión, CSRF, origen exacto y límites de cuerpo del Core siguen vigentes. Ver [OpenAPI](empresa-sar.openapi.yaml).

El formulario conserva los cambios ante fallos y conflictos. Cancelar restaura la copia cargada; recargar o navegar con cambios exige confirmación y no guarda. El estado numérico mostrado procede del servidor. No se cargan ni modifican datos fiscales reales como parte de la verificación del despliegue.

## Validación y despliegue — 2026-10-23

Instalado en AWS y Bluehost desde [`a85bca73`](https://github.com/Ahcast/Bastion/commit/a85bca73bcb90ec0d4dee6b91c815661aa432b57), tras [Core checks exitoso](https://github.com/Ahcast/Bastion/actions/runs/40064993129): formato y compilación Rust, pruebas con MySQL 5.7, 94 pruebas frontend y paquetes de producción. La prueba MySQL comprueba exclusividad administrativa incluso con grants Core de un rol personalizado, persistencia, rechazo de versión obsoleta y auditoría sin duplicar la escritura rechazada. Las pruebas de UI cubren guardado conjunto, conflictos, cancelación/recarga y normalización con versión estable.

Artifacts del mismo commit, ZIP/tar/archivos verificados por SHA-256, respaldo privado y despliegue con reversión. No hubo migraciones ni cambios de configuración del ambiente. Se verificó la conservación de datos comerciales, settings, documentos y activación de módulos. Salud HTTPS 200, ruta SAR sin sesión 401 y 35 archivos públicos verificados, incluido el bundle actual de Ajustes. El frontend conserva su origen API, cabeceras del hosting y assets anteriores.

La revisión visual local usó datos sintéticos. No se inició una sesión administrativa real ni se guardó numeración de ejemplo en el ambiente desplegado: la configuración SAR sigue sin configurar para que el administrador ingrese los datos de la empresa. Esto no valida una autorización fiscal ni habilita emisión, correlativos o documentos pendientes.


## Emisión desde Facturas — 2026-10-27

El usuario confirma que el número se reserva al marcar como enviado, no al crear borradores. El Core conserva una secuencia máxima por prefijo (`company.sar.sequence`), impide retrocesos de prefijos utilizados y avanza el ajuste junto a la factura en una transacción. Rango completo y fecha vigente se validan al emitir; los ajustes guardados no emiten documentos por sí solos. Ver [reglas y diferencias](modulos/facturas-acciones.md).
