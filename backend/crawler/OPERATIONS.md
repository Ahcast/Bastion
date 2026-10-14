# Bastion — crawler HonduCompras v1.0

Implementación de extracción y transporte basada en los tres scripts proporcionados y
el Documento Maestro Bastion v0.5. Python 3.12+, Requests, BeautifulSoup, SQLite.
Diseñado para Lightsail; objetivo inicial 2 GB RAM / 2 vCPU / 60 GB SSD. No desplegado ni validado en vivo.

## Responsabilidades
AWS observa y conserva estado técnico. Go en Lightsail y MySQL en Bluehost consolida identidad,
versiones, first_seen/last_seen, cambios y eventos AI. Este paquete NO instala ni
modifica el backend, MySQL, firewall o infraestructura AWS.

## Uso
```bash
python3.12 -m venv .venv
.venv/bin/pip install -r requirements.lock
cp .env.example .env
.venv/bin/python -m bastion_crawler run --dry-run
.venv/bin/python -m bastion_crawler run
.venv/bin/python -m bastion_crawler retry
.venv/bin/python -m bastion_crawler status
.venv/bin/python -m bastion_crawler upload
.venv/bin/python -m bastion_crawler validate data/batches/BATCH_ID
.venv/bin/python -m bastion_crawler replay data/batches/BATCH_ID
.venv/bin/python -m unittest discover -s tests -v
```
Dry-run consulta el sitio con las mismas pausas pero usa SQLite temporal y no entrega
ni cambia estado productivo. No puede predecir NEW/UPDATED sin la base maestra.
`replay` valida y reentrega el lote exacto; conserva Idempotency-Key. Para restauración
S3, descargar el directorio de lote bajo data/batches/BATCH_ID y sus raw bajo
 data/runs/RUN_ID/raw antes de reenviar a S3. No necesita volver a consultar el sitio.

## Cortesía y consumo
- Un único trabajador global (flock), sin concurrencia HTTP.
- 3 s como mínimo entre el final de una petición y el comienzo de la siguiente.
- Timeout de conexión/lectura 10/90 s, igual al script original.
- 3 intentos HTTP; espera 2/4 s más jitter, sin multiplicar retries de página.
- 3 unidades agotadas provocan pausa de 60 s; los detalles/documentos se detienen
  en esa pasada y quedan pendientes. Recuperación de unidades después de 15 min.
- Retry-After numérico se respeta; esperas mayores de 300 s se difieren a la cola.
- Reintento programado de pendientes cada hora. Una ejecución no se superpone a otra.
- Guardas de 4 horas, 500 páginas, 10 MB por respuesta y 2 GB de disco libre.
- No se recorre con navegadores, pandas, Redis, Celery o procesos permanentes.
- Se conservan vencidos y vacíos. En el orden que entrega HonduCompras, se procesa
  cada detalle y se detiene después de guardar tres procesos consecutivos cuya
  Fecha Recepción Ofertas corresponda al día calendario anterior en
  America/Tegucigalpa. Un vencimiento de hoy, anterior a ayer, ausente o ilegible,
  y un detalle fallido rompen la secuencia. El tercer proceso se incluye; no se
  consultan los detalles que le siguen ni las páginas siguientes. La página que
  contiene el tercer proceso ya tuvo que descargarse para enumerar sus enlaces.
- El motivo del corte queda en `runs.stop_reason` y en `run.json`; la corrida
  concluye normalmente con ese criterio, conserva `coverage=null` y explica el
  corte en `coverage_note`. No afirma haber recorrido el catálogo completo.
- `MAX_PAGES` y `MAX_RUN_SECONDS` siguen siendo guardas adicionales: alcanzarlos
  antes del criterio dejan el listado incompleto.

## Persistencia y resultados
Cada detalle confirmado se confirma en SQLite inmediatamente. No hay TRUNCATE.
La observación es única por run e identidad. Una nueva ejecución vuelve a observar
los procesos; la deduplicación entre ejecuciones pertenece a MySQL.
Cada cierre crea un lote inmutable con processes.csv, processes.json, run.json,
errors.json y manifest.json. No sobreescribe raw.csv ni procesos.csv del legacy.
CSV mantiene cuatro columnas originales y fechas legibles; no agrega resúmenes.
JSON transporta campos presentes, fechas ISO cuando se reconocen, observación UTC,
hash, identidad, URL y documentos. No contiene puntuación o análisis AI.
SHA-256 permite detectar cambios de documentos con el mismo nombre/URL.
HTML y documentos se guardan por contenido en raw/<sha256>.bin.
No se calculan NEW/UPDATED como verdad local ni se infieren eliminaciones.
La cobertura numérica permanece null cuando no se sabe el total; no se inventa.

## Entregas
Por defecto deshabilitadas hasta configurar el destino de prueba.
S3_ENABLED=true activa bucket existente us-east-1, privado, SSE-S3, checksum,
transferencias sin hilos y verificación posterior de tamaño/metadata SHA-256.
Credenciales por cadena estándar boto3; nunca dentro del repositorio.
BLUEHOST_MODE=api usa bearer token y los tres endpoints del documento maestro.
Máximo 100 procesos por solicitud, claves idempotentes y ACK de importación real.
BLUEHOST_MODE=ftps usa TLS verificado + prot_p, archivos temporales, checksum por
lectura posterior y nombres por lote en un inbox dedicado existente.
No escribe .htaccess ni reemplaza procesos.csv de producción. El importador debe
consumir JSON (CSV auxiliar) y generar BATCH_ID.ack.json; hasta entonces WAITING_ACK.
Un fallo de destino no vuelve a extraer datos. Cada destino se reintenta aparte.
No se borra automáticamente ninguna evidencia ni pendiente: establecer retención
tras comprobar importaciones y respaldo. La guarda de espacio evita agotar el disco.

## Límites conocidos y aceptación pendiente
- Pruebas offline con fixtures sintéticas; faltan HTML reales de página inicial,
  última página, cero resultados y pestañas de adjuntos.
- Se conserva el parser ASP.NET actual. Al reanudar paginación se reconstruye sesión
  y se recorren listados desde el inicio, saltando detalles ya completados. El
  contador de tres vencimientos consecutivos se reconstruye siguiendo el listado.
  Un índice de página no es un cursor estable del sitio.
- Descarga enlaces directos con extensiones documentales en el mismo host.
  Adjuntos por postback/JavaScript u otros dominios necesitan adaptar el parser.
  documents_complete=false evita falsas eliminaciones; no significa que no existan adjuntos.
- No hay backend Go en los archivos proporcionados. Su importador, versionado,
  ACK y cola AI deben implementarse según docs/IMPORT_CONTRACT.md.
- `status` y logs JSON proveen diagnóstico; las notificaciones externas no están
  configuradas. `replay` es de importación, no un comando para reparsear todo el RAW.
- No hubo conexión de prueba a HonduCompras, Bluehost, S3 ni medición en Lightsail.
  No se promete compatibilidad del importador legacy sin revisar su código.
- La URL HTTP original se conserva. Si el sitio ofrece HTTPS, verificarlo y cambiar
  SOURCE_URL. Redirecciones y descargas a otros hosts se rechazan explícitamente.

Consulte deploy/INSTALL.md para instalación paralela y rollback.

Códigos de salida de `run`: 0 extracción completa, 2 parcial, 1 fallida.
El estado de entrega se consulta aparte en `status`; éxito de extracción no implica
importación. `retry` procesa pendientes y registra el resultado en runs y deliveries.


## Destino actualizado

El receptor objetivo es la API Go en Lightsail; MySQL permanece en Bluehost. Configurar la URL API para ese receptor una vez implementado y validado. El identificador interno `bluehost` y el modo FTPS del cliente existente se conservan por compatibilidad con su estado SQLite y pruebas: no significan que el backend se ejecute allí. FTPS es legado, no transporte objetivo. No cambiar nombres persistidos sin migración ni activar entregas sin ACK de importación validado.

