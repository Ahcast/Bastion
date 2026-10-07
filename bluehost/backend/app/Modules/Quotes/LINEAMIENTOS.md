# Módulo Cotizaciones

## Objetivo

Mantener creación, cálculo, listado, búsqueda, estados, PDF, adjuntos, correo, clonación y conversiones, centralizando cálculos en backend y conectando IA de forma segura.

El módulo también debe aceptar la creación de cotizaciones borrador originadas desde Procesos, conservando trazabilidad hacia el expediente público que las originó.

## Funcionalidad legacy a conservar

### Listado
- búsqueda por número, orden, cliente e items/descripciones;
- paginación;
- filtros persistidos en URL;
- selección múltiple;
- acciones bulk;
- estados.

### Documento
- crear/editar;
- número automático `COT-...`;
- cliente registrado o manual;
- proceso/orden de referencia;
- fecha;
- días de validez;
- items;
- notas privadas;
- términos públicos;
- contacto del autor;
- clonar con nuevo correlativo.

### Cálculo
- cantidad;
- precio unitario;
- descuento por item en valor o porcentaje;
- descuento general;
- ISV 15%;
- ISV 18%;
- exento;
- exoneración;
- totales y redondeo.

La semántica legacy del descuento absoluto por item se conserva: **es por unidad**, no por línea.

### Adjuntos
- listar;
- subir;
- preview;
- descargar;
- soft-delete.

### PDF
- plantilla imprimible;
- logo;
- contacto;
- sello;
- total en letras;
- etiquetas de impuesto.

### Correo

Conservar:

```text
to
cc
bcc
subject
message
scheduled_at
PDF
adjuntos
```

Envío inmediato o programado por SMTP Bluehost.

### Conversiones
- cotización -> orden;
- cotización -> factura;
- equivalentes bulk.

## Debilidades legacy a eliminar

- cálculo duplicado PHP/TS;
- conversión a orden en navegador;
- pérdida de descuentos/impuestos;
- hard-delete sin papelera;
- PDF browser/base64 como dependencia;
- jobs JSON sin claim/lock;
- SMTP por sockets propios;
- adjuntos fuera de SQL;
- bulk como requests independientes sin lote trazable;
- referencia al proceso como texto aislado sin relación persistente.

## Cálculo canónico

Crear:

```text
QuoteCalculator
```

Debe resolver:

```text
descuento_unitario
subtotal_linea
descuento_general proporcional
base 15
base 18
exento
exonerado
ISV 15
ISV 18
total
```

Laravel persiste el resultado. Angular solo hace preview.

## Modelo objetivo

```text
quotes
quote_items
quote_counter
email_messages
documents
document_links
```

### Relación con Procesos

`quotes` debe incluir como mínimo:

```text
source_process_id nullable
process_reference_snapshot nullable
```

Además de los campos de snapshot del cliente:

```text
client_id nullable
client_name_snapshot
client_rtn_snapshot nullable
client_contact_snapshot nullable
client_email_snapshot nullable
client_address_snapshot nullable
```

Reglas:

- `source_process_id` referencia el proceso que originó la cotización;
- `process_reference_snapshot` conserva el expediente tal como se utilizó al crear la cotización;
- editar el proceso posteriormente no reescribe el snapshot de la cotización;
- una cotización manual puede tener `source_process_id = null`;
- un proceso puede originar varias cotizaciones;
- eliminar/anular una cotización no elimina ni modifica el proceso de origen.

Cardinalidad:

```text
processes 1 ---- N quotes
```

## Creación desde Procesos

La creación desde Procesos es una acción de integración entre módulos y debe ejecutarse en backend.

Entrada funcional:

```text
Proceso
- cliente / entidad
- expediente
```

Resultado:

```text
Cotización borrador
- cliente heredado/resuelto
- expediente heredado
- source_process_id
- items vacíos
```

No se transfieren automáticamente:

```text
especificaciones
fechas del proceso
documentos
notas personales
marks
score IA
raw crawler
items
precios
cantidades
```

### Resolución del cliente

La creación debe usar el resolver compartido de clientes:

```text
ClientResolver
```

Si existe match exacto/canónico:

```text
client_id = cliente encontrado
client_name_snapshot = nombre usado en la cotización
```

Si no existe match:

```text
client_id = null
client_name_snapshot = nombre de entidad del proceso
```

La cotización sigue siendo válida como borrador y la UI ofrece después:

```text
Vincular cliente
Crear cliente
```

No se crea automáticamente un cliente maestro desde HonduCompras.

### Action pública del módulo Quotes

El módulo debe exponer una Action/Service reutilizable, por ejemplo:

```text
CreateQuoteFromProcess
```

Esa Action debe ser utilizada por el endpoint del módulo Processes:

```http
POST /api/v1/processes/{process}/create-quote
```

De esta forma Processes solicita la operación, pero Quotes continúa siendo propietario de las reglas de creación de cotizaciones, numeración, snapshots y estado inicial.

### Estado inicial

Una cotización creada desde proceso comienza como:

```text
Borrador
```

No se marca como enviada ni convertida y no contiene items hasta que el usuario o una acción posterior autorizada los agregue.

### Idempotencia

La acción debe aceptar una llave idempotente para evitar duplicados accidentales por doble clic, timeout o retry.

La idempotencia protege una **misma intención**; no impide que un usuario cree deliberadamente otra cotización para el mismo proceso usando una nueva intención/llave.

## Navegación y UX

En el editor/detalle de una cotización originada desde Procesos debe mostrarse:

```text
Proceso de origen
ENEE-CM-GG-10005143-2026
[Ver proceso]
```

El expediente debe ser visible y editable solo según las reglas comerciales aprobadas, pero la relación técnica `source_process_id` no debe perderse por editar el texto visible.

El proceso muestra a su vez todas sus cotizaciones relacionadas.

## Numeración

El correlativo `COT-000001...` sigue siendo server-side con transacción, lock de contador y `UNIQUE quote_number`.

Crear una cotización desde un proceso usa exactamente el mismo mecanismo de numeración que crearla manualmente.

## API objetivo

```http
GET    /api/v1/quotes
POST   /api/v1/quotes
GET    /api/v1/quotes/{id}
PATCH  /api/v1/quotes/{id}
DELETE /api/v1/quotes/{id}

POST   /api/v1/quotes/{id}/clone
PATCH  /api/v1/quotes/{id}/status

POST   /api/v1/quotes/{id}/documents
GET    /api/v1/quotes/{id}/documents

POST   /api/v1/quotes/{id}/send
POST   /api/v1/quotes/{id}/schedule-send

POST   /api/v1/quotes/{id}/convert-to-order
POST   /api/v1/quotes/{id}/convert-to-invoice
```

Integración desde Procesos:

```http
POST /api/v1/processes/{process}/create-quote
```

Bulk:

```http
POST /api/v1/quotes/bulk/status
POST /api/v1/quotes/bulk/clone
POST /api/v1/quotes/bulk/export
```

Cada lote devuelve `batch_id` y resultado por item.

## Email Bluehost

```text
Quote
 -> snapshot PDF
 -> queued email
 -> Laravel Mail
 -> Bluehost SMTP
 -> sent/failed ledger
```

No guardar passwords SMTP por usuario en JSON. Programación mediante Scheduler/queue, nunca cron público.

## PDF

Objetivo:

```text
quote -> render -> PDF -> S3 -> document record -> email
```

El PDF enviado es el mismo snapshot registrado para ese envío.

Cuando la cotización tenga proceso de origen, el expediente visible forma parte de la información del documento según la plantilla aprobada, pero no se requiere incluir el identificador interno `source_process_id`.

## Conexiones

```mermaid
flowchart LR
    P[Procesos] -->|cliente + expediente| Q[Cotización borrador]
    P -. source_process_id .-> Q
    C[Clientes] --> Q
    Q -->|convertir| O[Órdenes]
    Q -->|convertir| F[Facturas]
    S3[(S3)] --> Q
```

## Seguridad

```text
cotizaciones.view
cotizaciones.create
cotizaciones.update
cotizaciones.delete
cotizaciones.send
cotizaciones.convert
```

Crear desde Proceso requiere como mínimo:

```text
procesos.view
cotizaciones.create
```

La Action `CreateQuoteFromProcess` vuelve a aplicar autorización y validación; no confía en que Angular haya ocultado o mostrado correctamente el botón.

## IA

Tools:

```text
get_quote
create_quote_draft
suggest_quote_items
compare_quote_with_process
calculate_margin
draft_quote_email
```

La IA crea borradores; enviar requiere policy/approval.

La creación básica desde un proceso **no depende de IA**.

Después de creada la cotización puede existir una acción separada:

```text
Generar items desde proceso con IA
```

Esa acción puede analizar las especificaciones del proceso y proponer items, pero los resultados son borradores/recomendaciones y no forman parte de la transferencia inicial cliente + expediente.

## Eventos

Al crear desde proceso se publican, después del commit, eventos compatibles con auditoría y automatizaciones:

```text
quote.created
process.quote_created
```

El payload debe identificar `process_id`, `quote_id` y actor, sin incluir secretos ni contenido raw innecesario.

## Criterios de aceptación

- Tests de regresión para descuentos/impuestos.
- Correlativo único.
- PDF, adjuntos y correo siguen funcionando.
- Programación no duplica envíos.
- Conversiones a orden/factura son server-side, idempotentes y trazables.
- Notas privadas no se imprimen.
- `terms` sigue siendo público.
- Un proceso puede crear una cotización borrador con cliente y expediente en una sola acción.
- Un proceso puede tener varias cotizaciones relacionadas.
- La relación `source_process_id` persiste aunque se edite el expediente visible.
- Cliente desconocido no bloquea la creación y queda como snapshot manual.
- La creación desde proceso no copia items, precios ni especificaciones automáticamente.
- La creación no depende de OpenAI ni de otro proveedor de IA.
