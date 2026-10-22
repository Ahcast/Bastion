# Cotizaciones: primer corte funcional en desarrollo

Fecha: 2026-10-22. Estado: primer corte publicado en el ambiente de pruebas; módulo incompleto y desactivado por defecto.

## Alcance implementado

Registro `cotizaciones`, migraciones propias y almacenamiento transaccional. Listado resumido con búsqueda por número, cliente, referencia, estado y descripción; consulta y edición con control de versión. Numeración automática `COT-000001`, vencimiento, estados visibles y cálculo de descuentos e impuestos. El contacto procede del usuario creador mediante el Core y se conserva al editar. La pantalla React carga bajo demanda y aporta una sección informativa propia en Ajustes.

La copia Legacy indicada por el usuario ya se cotejó. Ver [fuentes, fórmulas y diferencias comprobadas](cotizaciones-paridad.md), que reemplaza los supuestos del prototipo inicial. No se modificó esa copia ni se leyeron datos privados o secretos.

## Contrato y límites del corte

Ver `cotizaciones.openapi.yaml` (0.2.0). Permisos `cotizaciones/documentos/ver` y `cotizaciones/documentos/editar`, dentro de la matriz acordada. La copia del servidor no filtra por creador: los documentos del módulo son compartidos entre actores con permisos. La comprobación de paridad de permisos en sesión real sigue pendiente. No se presume multitenancy.

- Alta web: 30 días, término `Tiempo de entrega: `, estado Borrador y un ítem con cantidad 1, precio 0 e ISV 15%, conforme al editor activo. Los códigos `amount`/`percent` y `none`/`isv15`/`isv18` están confirmados. Se acepta `exempt` como alias de compatibilidad del prototipo, pero siempre se serializa `none`.
- Fecha más días para `valid_until`; vigencia 0..3650. Rango técnico de fechas: 1000-01-01..9999-12-31, sin vencimientos fuera de ese rango.
- Campos cotejados: cliente hasta 255; referencia hasta 100; descripción hasta 3000; notas hasta 3000; términos hasta 5000. Referencias de exoneración hasta 150 y reconocidas por el dominio, sin controles nuevos en la UI porque el frontend activo las fuerza a vacío.
- Cantidades/precios/descuentos como cadenas de hasta 12 enteros y 4 decimales de entrada; se normalizan a dos decimales antes de calcular/guardar. Los importes calculados también son cadenas. Nunca usar flotantes como fuente de verdad. Cantidad cero/descripción vacía permitidas; se descartan solo filas vacías con cantidad y precio cero. Descuentos acotados por las reglas del backend cotejado.
- Las operaciones aritméticas con enteros `i128` comprueban desbordamientos y rechazan entradas cuyo cálculo exceda su rango. Los máximos de entrada y 200 ítems son límites técnicos, no límites observados del Legacy.
- Listado: página 1..10000, 1..100 resúmenes (25 por defecto), búsqueda hasta 200 caracteres; fecha e ID descendentes. El detalle carga ítems y textos completos. Son límites técnicos del contrato nuevo.
- Cuerpo hasta 1 MiB; 120 solicitudes por actor/minuto, timeout HTTP 30 segundos. La vista previa POST `/calculate` comparte cuota, sesión y CSRF, requiere editar y no persiste.
- POST de creación exige `Idempotency-Key` de 16..128 caracteres ASCII alfanuméricos, guion o guion bajo. Clave por actor; misma solicitud devuelve el resultado original, cuerpo diferente produce 409. Documento, incremento del contador, recibo y auditoría se guardan juntos. Editar conserva número. Los borradores del prototipo sin número lo reciben al guardar por primera vez.
- PUT exige una versión fuerte `If-Match`; versión obsoleta produce 412; ausente/malformada 422. El estado acepta solo Borrador/Enviada/Aprobada/Rechazada/Convertido. El formulario conserva la edición de estado observada; no se inventa una matriz de transiciones.
- Totales y numeración no son editables desde el cliente. Los overrides de contacto/preparador se ignoran: se obtienen del Core al crear y se conserva la instantánea original al editar.
- Las escrituras bloquean la habilitación para coordinarse con la desactivación administrativa y auditan dentro de la transacción. La desactivación conserva documentos, contador y recibos.

## Cálculo y paridad

Rust reproduce la secuencia de redondeo del backend guardado: descuento unitario, descuento general prorrateado por línea, bases por tipo de impuesto e impuestos sobre bases agrupadas. La vista previa web llama ese mismo cálculo, sin duplicarlo en JavaScript. No se reparte el residuo del descuento general de forma nueva. El significado legado del total por ítem también se conserva; ver el documento de paridad.

Queda pendiente reconciliar los casos límite de redondeo con ejemplos contables anonimizados. Esta revisión estática no constituye validación contable ni equivalencia visual verificada.

## Activación y siguientes etapas

`migrate` aplica Core, Procesos y Cotizaciones con ledgers independientes. `serve` comprueba versiones/checksums sin crear tablas. `BASTION_ALLOWED_MODULES` admite IDs separados por comas (`procesos,cotizaciones`); vacío mantiene Core solo. Además hacen falta migraciones compatibles y activación administrativa.

El contador nuevo empieza en cero y no implica continuidad con producción. Antes de migrar cotizaciones del Legacy se deben reconciliar números, referencias, importes, contactos y archivos. Este corte no migra datos.

Pendientes: catálogo Clientes, PDF/Typst y plantilla, adjuntos privados, correo inmediato/programado, clonación, eliminación confirmada, conversiones, acciones masivas y MCP. No se exponen botones de esas operaciones antes de implementarlas. Los settings propios siguen pendientes y Ajustes lo indica.

El usuario autorizó la publicación en GitHub y AWS. El corte técnico se desplegó con respaldo previo; esto no lo convierte en módulo terminado ni en aprobación de producción.

## Verificación

Tipos TypeScript, 35 pruebas del frontend y build Vite verificados; rustfmt y metadatos Cargo verificados. Se añaden pruebas Rust para fórmulas mixtas, descuento unitario, redondeo agrupado, prorrateo sin redistribución, normalización, exoneración y límites. La prueba de integración MySQL contempla permisos, idempotencia concurrente, numeración estable, contacto del Core, edición concurrente, auditoría, búsqueda literal y desactivación, en una base CI aislada.

Rust, pruebas unitarias e integración con MySQL 5.7 pasaron en GitHub Actions. El compilador portátil local sigue sin biblioteca estándar; la validación ejecutada en CI cierra esa comprobación del release. La verificación autenticada del editor y la reconciliación contable siguen pendientes.

## Publicación técnica verificada — 2026-10-22

Código ejecutado: `78743be826c4d57ea9e46f6459fa912566c50241`; [CI aprobado](https://github.com/Ahcast/Bastion/actions/runs/39824576638). Se corrigieron durante CI la vida útil de los identificadores de la allowlist y la lectura con tipo firmado del resultado `SELECT 1` de autorización.

Backend instalado en AWS Lightsail y frontend en Bluehost desde artifacts del mismo commit, con procedencia y hashes SHA-256 comprobados. Se crearon respaldos privados de la base y del frontend. Las dos migraciones de Cotizaciones se aplicaron; los 835 procesos y los conteos de datos existentes se conservaron. API HTTPS: health live/ready 200; consulta anónima de Cotizaciones/Procesos 401. Se verificaron por HTTPS los hashes de HTML/assets y la carga de la pantalla de acceso sin errores de consola.

Cotizaciones quedó incluido y permitido por el ambiente, **sin activación administrativa**. El Administrador puede activarlo en Ajustes > Core > Módulos. Se conservó la activación de Procesos y la recepción automática de Scout siguió desactivada. Rutas internas y respaldos reales permanecen en los registros privados de despliegue.
